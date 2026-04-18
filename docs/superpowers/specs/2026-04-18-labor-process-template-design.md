# 工序模板管理 — 设计文档

## 背景

当前人工成本通过 `bom_labor_process` 表按 `product_code` 逐个 BOM 单独设置。缺点是工序价格变动时需要手动更新所有相关 BOM。

本设计引入「工序模板」概念，将工序定义与 BOM 解耦：工序价格统一在模板中维护，BOM 通过引用模板步骤来计算人工成本。

## 需求

- 三层结构：工序组 → 工序分类 → 工序步骤（每个步骤有独立价格）
- BOM 关联一个工序组，粒度到步骤级别设置数量
- 纯引用：BOM 不存价格，实时从模板取价
- 被引用的工序组/分类/步骤禁止删除
- 全新表结构，不迁移旧数据

## 数据库 Schema

### `labor_process_group`（工序组）

```sql
CREATE TABLE labor_process_group (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    remark TEXT,
    created_by VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

COMMENT ON TABLE labor_process_group IS '工序组';
COMMENT ON COLUMN labor_process_group.name IS '工序组名称（唯一），如"电源工序"、"模组工序"';
COMMENT ON COLUMN labor_process_group.created_by IS '创建人';
```

### `labor_process_item`（工序项 — 分类 + 步骤合一）

```sql
CREATE TABLE labor_process_item (
    id BIGSERIAL PRIMARY KEY,
    group_id BIGINT NOT NULL,  -- 关联 labor_process_group.id
    parent_id BIGINT,  -- NULL=分类, 非NULL=步骤(指向分类id)
    name VARCHAR(255) NOT NULL,
    unit_price DECIMAL(18,6),             -- 分类为NULL, 步骤有值
    sort_order INT NOT NULL DEFAULT 0,
    created_by VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    CHECK (
        (parent_id IS NULL AND unit_price IS NULL) OR
        (parent_id IS NOT NULL AND unit_price IS NOT NULL)
    ),
    UNIQUE(group_id, parent_id, name)     -- 同组同分类下名称唯一
);

CREATE INDEX idx_lpi_group_parent ON labor_process_item(group_id, parent_id);

COMMENT ON TABLE labor_process_item IS '工序项（分类和步骤合一，通过 parent_id 区分）';
COMMENT ON COLUMN labor_process_item.parent_id IS 'NULL=分类, 非NULL=步骤(指向分类id，外键约束保证引用有效)';
COMMENT ON COLUMN labor_process_item.unit_price IS '分类为NULL, 步骤有值';
```

关键设计决策：
- **parent_id 使用 NULL 而非 0 哨兵值**：NULL 语义更清晰地表达"无父节点"，查询条件 `parent_id IS NULL` 比 `parent_id = 0` 更符合 SQL 惯例。引用完整性由应用层保证。
- **DECIMAL(18,6)** 与项目已有约定一致（migration 011 统一了全系统精度）。
- **UNIQUE(group_id, parent_id, name)** 防止同组同分类下出现重名步骤。
- **created_by** 满足项目约定的审计追踪要求（operator_id tracking）。
- **无外键约束**：不使用 REFERENCES，引用完整性由应用层逻辑保证。

### `bom_labor_process_ref`（BOM 工序引用）

```sql
CREATE TABLE bom_labor_process_ref (
    id BIGSERIAL PRIMARY KEY,
    bom_id BIGINT NOT NULL,  -- 关联 bom.bom_id
    step_id BIGINT NOT NULL,  -- 关联 labor_process_item.id
    quantity DECIMAL(18,6) NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    UNIQUE(bom_id, step_id)
);

CREATE INDEX idx_blpr_bom ON bom_labor_process_ref(bom_id);
CREATE INDEX idx_blpr_step ON bom_labor_process_ref(step_id);

COMMENT ON TABLE bom_labor_process_ref IS 'BOM 工序引用（引用工序模板步骤，实时取价）';
COMMENT ON COLUMN bom_labor_process_ref.step_id IS '引用 labor_process_item 中的步骤（parent_id IS NOT NULL）';
COMMENT ON COLUMN bom_labor_process_ref.quantity IS '工序数量，0 表示不使用该步骤';
```

表名使用 `bom_labor_process_ref`（ref = reference）而非 `_new` 后缀，语义明确表达这是引用关系表。

### 旧表处理

旧的 `bom_labor_process` 表保留不删，旧 proto 接口保留不动，新旧互不干扰。

## gRPC 接口

### `LaborProcessGroupService`（新 service）

```protobuf
rpc ListProcessGroups(ListProcessGroupsRequest) returns (ProcessGroupListResponse);
rpc GetProcessGroup(GetProcessGroupRequest) returns (ProcessGroupDetailResponse);
rpc CreateProcessGroup(CreateProcessGroupRequest) returns (U64Response);
rpc UpdateProcessGroup(UpdateProcessGroupRequest) returns (BoolResponse);
rpc DeleteProcessGroup(DeleteProcessGroupRequest) returns (BoolResponse);
```

- `ListProcessGroups` 支持分页和搜索：`{ keyword, page, page_size }`
- `GetProcessGroup` 返回完整树结构（组 → 分类 → 步骤），前端一次拿到全部数据
- `DeleteProcessGroup` 检查是否有 item 被 BOM 引用，有则拒绝

### `LaborProcessItemService`（新 service）

```protobuf
rpc CreateProcessItem(CreateProcessItemRequest) returns (U64Response);
rpc UpdateProcessItem(UpdateProcessItemRequest) returns (BoolResponse);
rpc DeleteProcessItem(DeleteProcessItemRequest) returns (BoolResponse);
rpc SwapProcessItem(SwapProcessItemRequest) returns (BoolResponse);
```

- 创建时通过 `parent_id` 区分是分类（NULL）还是步骤（非 NULL）
- 删除分类时检查该分类下所有步骤是否被 BOM 引用（不只是检查分类自身）
- 删除步骤时直接检查 `bom_labor_process_ref` 引用
- 如果分类下有步骤被引用，拒绝删除整个分类

### BOM 工序引用（添加到现有 `AbtBomService`）

```protobuf
rpc SetBomLaborProcess(SetBomLaborProcessRequest) returns (BoolResponse);
rpc GetBomLaborProcess(GetBomLaborProcessRequest) returns (BomLaborProcessDetailResponse);
```

`SetBomLaborProcessRequest`:
- `bom_id`: 目标 BOM
- `group_id`: 选中的工序组
- `steps`: repeated `StepQuantity { step_id, quantity }`

`GetBomLaborProcessResponse`:
- 返回完整工序树 + 每个步骤的 quantity 和 subtotal

`SetBomLaborProcess` 语义定义：
- **幂等替换**：每次调用完全替换该 BOM 的工序列配置（事务内先删旧引用再批量插入新引用）
- **step_id 归属校验**：repository 层执行 `SELECT ... WHERE id IN (step_ids) AND group_id = ?`，验证所有 step_id 属于指定 group_id，不一致则返回 `INVALID_ARGUMENT` 并附上无效的 step_id 列表
- **切换工序组**：直接传入新 group_id + 新 steps 即可，旧的引用在事务中被清除

### BOM 响应扩展

在现有 `BomResponse` / `BomListResponse` 中增加工序组关联信息：
- `optional int64 labor_process_group_id` — 关联的工序组 ID（未关联则为空）
- `optional string labor_process_group_name` — 工序组名称

前端在 BOM 列表页可直接看到每个 BOM 的工序配置状态，无需逐个查询。

## 服务层 & 数据流

### 文件结构

```
models/
  labor_process_group.rs   — ProcessGroup struct
  labor_process_item.rs    — ProcessItem struct
  bom_labor_process_ref.rs — BomLaborProcessRef struct

repositories/
  labor_process_group_repo.rs  — 工序组 CRUD
  labor_process_item_repo.rs   — 工序项 CRUD + find_full_tree
  bom_labor_process_ref_repo.rs — BOM 工序引用 CRUD

service/
  labor_process_group_service.rs
  labor_process_item_service.rs
implt/
  labor_process_group_service_impl.rs
  labor_process_item_service_impl.rs
```

BOM 工序引用逻辑放在现有 `BomService` 中。

### 关键查询

- `find_full_tree(group_id)` → 查询该组所有 item，应用层组装树
- `is_step_referenced(step_id)` → 检查步骤是否被 BOM 引用
- `is_category_referenced(category_id)` → 检查分类下所有步骤是否被引用
- `is_group_referenced(group_id)` → 检查组是否有任意步骤被引用
- `replace_bom_processes(bom_id, group_id, steps[])` → 事务内删旧 + 批量插入新 + step_id 归属校验
- `get_bom_labor_group(bom_id)` → 查询 BOM 关联的工序组信息（用于列表页展示）

### 核心流程

**删除工序组：**
收到删除请求 → 查询该组所有 item 的 id → 查询 `bom_labor_process_ref` 是否有任何引用 → 有则返回错误（附引用的 BOM 列表） → 无则删除所有 item + 组

**删除工序分类：**
收到删除请求 → 查询该分类下所有步骤 → 检查这些步骤是否被 BOM 引用 → 有则返回错误 → 无则先删所有子步骤再删分类

**删除工序步骤：**
收到删除请求 → 查询 `bom_labor_process_ref` 是否引用该步骤 → 有则返回错误 → 无则删除

**BOM 设置工序：**
收到请求 → 校验所有 step_id 属于指定 group_id → 事务开始 → 删除旧引用 → 批量插入新引用 → 事务提交

**查询 BOM 人工成本：**
查询 BOM 引用 → 查询步骤价格 → 计算 subtotal → 组装树返回

**查询 BOM 列表（含工序信息）：**
在 ListBoms 查询中 LEFT JOIN `bom_labor_process_ref` + `labor_process_item` + `labor_process_group`，获取关联的工序组 ID 和名称。

## 新增文件清单

```
新增迁移:
  abt/migrations/021_labor_process_group.sql
  abt/migrations/022_labor_process_item.sql
  abt/migrations/023_bom_labor_process_ref.sql

新增 proto:
  proto/abt/v1/labor_process.proto
  proto/abt/v1/bom.proto (追加 rpc、message 和 BomResponse 扩展字段)

新增代码:
  abt/src/models/labor_process_group.rs
  abt/src/models/labor_process_item.rs
  abt/src/models/bom_labor_process_ref.rs
  abt/src/repositories/labor_process_group_repo.rs
  abt/src/repositories/labor_process_item_repo.rs
  abt/src/repositories/bom_labor_process_ref_repo.rs
  abt/src/service/labor_process_group_service.rs
  abt/src/service/labor_process_item_service.rs
  abt/src/implt/labor_process_group_service_impl.rs
  abt/src/implt/labor_process_item_service_impl.rs
  abt-grpc/src/handlers/labor_process_group.rs
  abt-grpc/src/handlers/labor_process_item.rs

修改:
  abt/src/lib.rs (添加工厂函数)
  abt-grpc/src/server.rs (注册新 service)
  abt/src/models/mod.rs (注册新 model)
  abt/src/repositories/mod.rs (注册新 repo)
  abt/src/service/mod.rs (注册新 service trait)
  abt/src/implt/mod.rs (注册新 impl)
  abt-grpc/src/handlers/mod.rs (注册新 handler)
```
