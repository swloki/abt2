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
    name VARCHAR(255) NOT NULL,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

COMMENT ON TABLE labor_process_group IS '工序组';
COMMENT ON COLUMN labor_process_group.name IS '工序组名称，如"电源工序"、"模组工序"';
```

### `labor_process_item`（工序项 — 分类 + 步骤合一）

```sql
CREATE TABLE labor_process_item (
    id BIGSERIAL PRIMARY KEY,
    group_id BIGINT NOT NULL REFERENCES labor_process_group(id),
    parent_id BIGINT NOT NULL DEFAULT 0,
    name VARCHAR(255) NOT NULL,
    unit_price DECIMAL(12,2),
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    CHECK (
        (parent_id = 0 AND unit_price IS NULL) OR
        (parent_id > 0 AND unit_price IS NOT NULL)
    )
);

CREATE INDEX idx_lpi_group ON labor_process_item(group_id);
CREATE INDEX idx_lpi_parent ON labor_process_item(parent_id);

COMMENT ON TABLE labor_process_item IS '工序项（分类和步骤合一，通过 parent_id 区分）';
COMMENT ON COLUMN labor_process_item.parent_id IS '0=分类, >0=步骤(指向分类id)';
COMMENT ON COLUMN labor_process_item.unit_price IS '分类为NULL, 步骤有值';
```

CHECK 约束确保分类没有价格、步骤必须有价格。

### `bom_labor_process_new`（BOM 工序引用）

```sql
CREATE TABLE bom_labor_process_new (
    id BIGSERIAL PRIMARY KEY,
    bom_id BIGINT NOT NULL REFERENCES bom(bom_id),
    step_id BIGINT NOT NULL REFERENCES labor_process_item(id),
    quantity DECIMAL(12,2) NOT NULL DEFAULT 1,
    UNIQUE(bom_id, step_id)
);

CREATE INDEX idx_blpn_bom ON bom_labor_process_new(bom_id);
CREATE INDEX idx_blpn_step ON bom_labor_process_new(step_id);

COMMENT ON TABLE bom_labor_process_new IS 'BOM 工序引用（引用工序模板步骤）';
COMMENT ON COLUMN bom_labor_process_new.step_id IS '引用 labor_process_item 中的步骤（parent_id > 0）';
COMMENT ON COLUMN bom_labor_process_new.quantity IS '工序数量，0 表示不使用该步骤';
```

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

- `GetProcessGroup` 返回完整树结构（组 → 分类 → 步骤）
- `DeleteProcessGroup` 检查是否有 item 被 BOM 引用，有则拒绝

### `LaborProcessItemService`（新 service）

```protobuf
rpc CreateProcessItem(CreateProcessItemRequest) returns (U64Response);
rpc UpdateProcessItem(UpdateProcessItemRequest) returns (BoolResponse);
rpc DeleteProcessItem(DeleteProcessItemRequest) returns (BoolResponse);
rpc SwapProcessItem(SwapProcessItemRequest) returns (BoolResponse);
```

- 通过 `parent_id` 区分分类和步骤
- 删除时检查引用

### BOM 工序引用（添加到 `AbtBomService`）

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

## 服务层 & 数据流

### 文件结构

```
models/
  labor_process_group.rs   — ProcessGroup struct
  labor_process_item.rs    — ProcessItem struct
  bom_labor_process_new.rs — BomLaborProcessNew struct

repositories/
  labor_process_group_repo.rs  — 工序组 CRUD
  labor_process_item_repo.rs   — 工序项 CRUD + find_full_tree
  bom_labor_process_new_repo.rs — BOM 工序引用 CRUD

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
- `is_referenced_by_bom(step_id)` → 检查步骤是否被 BOM 引用
- `is_group_referenced_by_bom(group_id)` → 检查组是否有步骤被引用
- `replace_bom_processes(bom_id, steps[])` → 事务内删旧 + 批量插入新

### 核心流程

**删除工序组/分类/步骤：**
收到删除请求 → 查询 `bom_labor_process_new` 是否有引用 → 有则返回错误 → 无则删除

**BOM 设置工序：**
收到请求 → 事务开始 → 删除旧引用 → 批量插入新引用 → 事务提交

**查询 BOM 人工成本：**
查询 BOM 引用 → 查询步骤价格 → 计算 subtotal → 组装树返回

## 新增文件清单

```
新增迁移:
  abt/migrations/021_labor_process_group.sql
  abt/migrations/022_labor_process_item.sql
  abt/migrations/023_bom_labor_process_new.sql

新增 proto:
  proto/abt/v1/labor_process.proto
  proto/abt/v1/bom.proto (追加 rpc 和 message)

新增代码:
  abt/src/models/labor_process_group.rs
  abt/src/models/labor_process_item.rs
  abt/src/models/bom_labor_process_new.rs
  abt/src/repositories/labor_process_group_repo.rs
  abt/src/repositories/labor_process_item_repo.rs
  abt/src/repositories/bom_labor_process_new_repo.rs
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
