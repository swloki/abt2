# BOM Detail: JSONB → 独立表迁移设计

## 背景

当前 `bom` 表的 `bom_detail` 是一个 JSONB 列，存储 BOM 的完整节点树结构。业务需要将 BOM 节点拆分到独立的 `bom_nodes` 表中，每行对应一个 BOM 节点，通过 `bom_id` 关联到 `bom` 表。

## 数据库 Schema

### 新建 `bom_nodes` 表

```sql
CREATE TABLE bom_nodes (
    id          BIGSERIAL PRIMARY KEY,
    bom_id      BIGINT NOT NULL,
    product_id  BIGINT NOT NULL,
    product_code VARCHAR(255),
    quantity    DECIMAL(10,6) NOT NULL,
    parent_id   BIGINT,
    loss_rate   DECIMAL(10,6) NOT NULL DEFAULT 0,
    "order"     INT NOT NULL DEFAULT 0,
    unit        VARCHAR(50),
    remark      TEXT,
    position    VARCHAR(255),
    work_center VARCHAR(255),
    properties  TEXT
);

CREATE INDEX idx_bom_nodes_bom_id ON bom_nodes(bom_id);
CREATE INDEX idx_bom_nodes_parent_id ON bom_nodes(parent_id);
CREATE INDEX idx_bom_nodes_product_id ON bom_nodes(product_id);
```

设计决策：
- 不使用外键约束，只保留索引保证查询性能
- `parent_id` 根节点为 `NULL`（旧数据中 `parent_id = 0` 表示根节点，迁移时转为 `NULL`）
- `id` 全局自增，不再按 BOM 内局部编号
- `quantity` 和 `loss_rate` 用 `DECIMAL(10,6)` 保持精度一致
- 不加 `created_at` / `updated_at`，时间戳在 `bom` 表层面管理

### `bom` 表变更

- 添加 `created_by BIGINT` 列（从 `bom_detail.created_by` 迁移）
- 保留 `bom_detail` JSONB 列（后续稳定后再删）

## TypeScript 迁移脚本

位置：`scripts/migrate-bom-detail.ts`

逻辑：
1. 从 `bom` 表读取所有记录的 `bom_id` 和 `bom_detail`
2. 解析每个 `bom_detail.nodes` 数组
3. 将 `created_by` 写入 `bom` 表的新 `created_by` 列
4. 两阶段插入 `bom_nodes`：
   - 第一轮：插入所有节点，生成新的 `id`，`parent_id = NULL` 临时占位
   - 记录映射关系：旧 id → 新 id
   - 第二轮：更新 `parent_id`，将旧 parent_id 替换为新的 id（根节点保持 `NULL`）
5. 处理 `bom_detail` 为空或格式异常的数据

## Rust 代码变更

### Model 层（`abt/src/models/`）

- 新增 `BomNode` 结构体，字段对应 `bom_nodes` 表
- 新增 `NewBomNode`（插入用）和 `UpdateBomNode`（更新用）
- `Bom` 结构体中 `bom_detail: BomDetail` 改为通过关联查询获取
- `BomDetail` 结构体废弃

### Repository 层（`abt/src/repositories/`）

新增 `BomNodeRepo`：
- `insert(bom_id, node)` — 插入节点
- `batch_insert(bom_id, nodes)` — 批量插入
- `find_by_bom_id(bom_id)` — 查询 BOM 的所有节点
- `find_by_id(id)` — 查询单个节点
- `update(id, node)` — 更新节点
- `delete(id)` — 删除节点（含递归删除子节点）
- `find_by_product_id(product_id)` — 按产品查节点

`BomRepo` 去掉所有 `jsonb_array_elements()` 相关查询。

### Service 层

- `BomService` trait 方法签名不变，保持 gRPC 接口稳定
- 内部实现从操作 JSONB 改为调用 `BomNodeRepo`
- `add_node` / `update_node` / `delete_node` / `get_leaf_nodes` 改为 SQL 操作
- `substitute_product` 改为直接 UPDATE `bom_nodes` WHERE `product_id = ?`

### Handler 层

- proto 定义不变，gRPC 接口保持兼容
- 调整 model ↔ proto 转换逻辑，去掉 JSONB 序列化/反序列化

### 工厂函数（`abt/src/lib.rs`）

- 新增 `get_bom_node_service(ctx)` 工厂函数

## 迁移流程

1. Rust migration — 创建 `bom_nodes` 表 + `bom` 表加 `created_by` 列
2. TypeScript 迁移脚本 — JSONB 数据迁移到 `bom_nodes`，回填 `created_by`
3. Rust 代码重写 — 全面切到 `bom_nodes` 表
4. 部署验证 — 确认功能正常
5. 后续清理（不在本次范围）— 稳定后删除 `bom_detail` 列
