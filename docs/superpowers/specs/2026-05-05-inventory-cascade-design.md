# 级联查询库存功能设计

## 概述

给定一个产品 ID 或 product_code，查出该产品被哪些 BOM 引用，并按 BOM 分别展示该产品节点的直接子节点及其库存总量。

## 方案选择

采用两次查询方案：
1. 一次查询拿到顶层产品信息 + 所有 BOM 引用及子节点（CTE 合并产品查找，减少 roundtrip）
2. 批量查询子节点产品的库存汇总

避免单次大 JOIN（复杂度高）和无界递归（当前只需直接子节点，过度设计）。

## Proto 定义

新增文件 `proto/abt/v1/inventory_cascade.proto`：

```protobuf
message CascadeInventoryRequest {
  oneof product_identifier {
    int64 product_id = 1;
    string product_code = 2;
  }
  optional int32 max_results = 3;  // 默认 500，上限 2000
}

message BomCascadeGroup {
  int64 bom_id = 1;
  string bom_name = 2;
  repeated ChildNodeInventory children = 3;
}

message ChildNodeInventory {
  int64 node_id = 1;
  int64 product_id = 2;
  string product_code = 3;
  string product_name = 4;
  string unit = 5;
  double quantity = 6;
  double total_stock = 7;
  double loss_rate = 8;
  int32 order = 9;             // 在 BOM 中的排序
  optional int64 parent_node_id = 10;  // 所属父节点 ID
}

message CascadeInventoryResponse {
  int64 product_id = 1;
  string product_code = 2;
  string product_name = 3;
  repeated BomCascadeGroup bom_groups = 4;
}
```

在现有 inventory service 中添加 rpc：

```protobuf
rpc CascadeInventory(CascadeInventoryRequest) returns (CascadeInventoryResponse);
```

## 数据流

1. **第一次查询**：CTE 一次拿到顶层产品信息 + BOM 引用 + 子节点结构（减少 roundtrip）

```sql
WITH parent_product AS (
  SELECT product_id, product_code, pdt_name
  FROM products
  WHERE (product_id = $1 OR product_code = $2)
    AND deleted_at IS NULL
  LIMIT 1
)
SELECT
  pp.product_id AS root_product_id,
  pp.product_code AS root_product_code,
  pp.pdt_name AS root_product_name,
  b.bom_id,
  b.bom_name,
  child.id AS node_id,
  child.product_id,
  child.product_code,
  p_child.pdt_name AS product_name,
  child.unit,
  child.quantity,
  child.loss_rate,
  child."order",
  bn_parent.id AS parent_node_id
FROM parent_product pp
JOIN bom_nodes bn_parent ON bn_parent.product_id = pp.product_id
JOIN bom b ON b.bom_id = bn_parent.bom_id
          AND b.deleted_at IS NULL
JOIN bom_nodes child ON child.parent_id = bn_parent.id
                     AND child.bom_id = bn_parent.bom_id
JOIN products p_child ON p_child.product_id = child.product_id
                      AND p_child.deleted_at IS NULL
ORDER BY b.bom_id, child."order"
LIMIT $3;
```

注意：不加 `bn_parent.parent_id IS NULL`，因为产品可能出现在 BOM 的任意层级，不仅限于根节点。

注意：如果一个产品在同一个 BOM 中作为多个节点的子节点出现，会产生多行相同 bom_id 的记录。当前设计按 bom_id 分组时会合并到同一个 BomCascadeGroup 中，不做去重——同一 BOM 中该产品不同位置的子节点都会列出。

若 CTE 返回空（产品不存在），直接返回 NOT_FOUND。若返回有顶层产品但无子行（未被 BOM 引用），返回空 bom_groups。

2. **第二次查询**：批量获取子节点产品的库存汇总

```sql
SELECT
  i.product_id,
  SUM(i.quantity) AS total_stock
FROM inventory i
WHERE i.product_id = ANY($1)
GROUP BY i.product_id
```

3. **组装结果**（Service 层职责）：
   - Repository 返回扁平查询结果，不做分组
   - Service 用 `HashMap<i64, Decimal>` 存库存汇总（无记录的默认为 0），合并到子节点
   - 用 `HashMap<i64, BomCascadeGroup>` 按 `bom_id` 聚合，避免 Vec 反复遍历
   - 组装 `CascadeInventoryResult`

## 新增文件

| 文件 | 说明 |
|------|------|
| `proto/abt/v1/inventory_cascade.proto` | Proto 定义 |
| `abt/src/models/inventory_cascade.rs` | Model 结构体 |
| `abt/src/repositories/inventory_cascade_repo.rs` | 两次 SQL 查询 |
| `abt/src/service/inventory_cascade_service.rs` | Service trait |
| `abt/src/implt/inventory_cascade_service.rs` | Service 实现 |
| `abt-grpc/src/handlers/inventory_cascade_handler.rs` | gRPC handler |

## 修改文件

| 文件 | 说明 |
|------|------|
| `abt/src/models/mod.rs` | 注册新 model module |
| `abt/src/repositories/mod.rs` | 注册新 repo module |
| `abt/src/service/mod.rs` | 注册新 service trait module |
| `abt/src/implt/mod.rs` | 注册新 impl module |
| `abt/src/lib.rs` | 添加工厂函数 `get_inventory_cascade_service` |
| `abt-grpc/src/handlers/mod.rs` | 注册新 handler module |
| `abt-grpc/src/server.rs` | 注册 rpc 到 server |
| `proto/abt/v1/inventory.proto` (或其他 proto) | 添加 rpc 定义 |

## Model 结构体

```rust
pub struct CascadeInventoryResult {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub bom_groups: Vec<BomCascadeGroup>,
}

pub struct BomCascadeGroup {
    pub bom_id: i64,
    pub bom_name: String,
    pub children: Vec<ChildNodeInventory>,
}

pub struct ChildNodeInventory {
    pub node_id: i64,
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub unit: Option<String>,
    pub quantity: Decimal,
    pub total_stock: Decimal,  // 默认 0，不使用 Option
    pub loss_rate: Decimal,
    pub order: i32,
    pub parent_node_id: Option<i64>,
}
```

## Service 接口

```rust
#[async_trait]
pub trait InventoryCascadeService: Send + Sync {
    async fn cascade_inventory(
        &self,
        product_id: Option<i64>,
        product_code: Option<String>,
    ) -> anyhow::Result<CascadeInventoryResult>;
}
```

Service 实现逻辑：
- Repository 返回扁平查询结果，不做分组（Repo 保持"纯查询"职责）
- Service 调用 repo 两次查询
- 库存查询无记录时 total_stock 默认为 0，不使用 Option
- 用 `HashMap<i64, BomCascadeGroup>` 按 bom_id 聚合，避免 Vec 反复遍历
- loss_rate 只返回原始值，不做"损耗后需求量"计算（如需后续可加字段）

## 边界情况

| 场景 | 处理方式 |
|------|----------|
| 产品不存在 / 已软删除 | 返回 gRPC NOT_FOUND |
| 产品没有被任何 BOM 引用 | 返回空 bom_groups |
| 产品在 BOM 中是叶子节点（无子节点） | 该 BOM 的 children 为空数组 |
| 子节点产品无库存记录 | total_stock 返回 0.0 |
| BOM 已软删除 | SQL JOIN 条件中 `b.deleted_at IS NULL` 过滤 |
| 数据库查询失败 | 返回 gRPC INTERNAL，记录错误日志 |
| 同一产品在同一 BOM 中出现在多个节点 | 所有位置的子节点都列出，不做去重 |
| product_id 和 product_code 都为空 | 返回 gRPC INVALID_ARGUMENT |
| 同时传入 product_id 和 product_code | product_id 优先 |

## 错误处理 & 日志

- 输入校验：`product_id` 和 `product_code` 都为空 → `INVALID_ARGUMENT`；同时传入时 `product_id` 优先
- 产品不存在：`NOT_FOUND`，附带产品 ID/code 信息
- 数据库错误：`INTERNAL`，不透传 anyhow，记录 `error!` 级别日志
- 查询耗时：分别记录结构查询和库存查询的耗时（`info!` 级别），便于性能监控

## 索引要求

确保以下索引存在（如不存在需在迁移中添加）：

```sql
CREATE INDEX IF NOT EXISTS idx_bom_nodes_product_bom_parent
  ON bom_nodes(product_id, bom_id, parent_id);
CREATE INDEX IF NOT EXISTS idx_bom_nodes_parent_bom_order
  ON bom_nodes(parent_id, bom_id, "order");
CREATE INDEX IF NOT EXISTS idx_inventory_product
  ON inventory(product_id);
```

`bom(bom_id, deleted_at)` 主键已覆盖，无需额外索引。

## 性能考虑

- **结果集可配置限制**：通过 `max_results` 参数控制（默认 500，上限 2000），防止产品被大量 BOM 引用时返回过大结果集
- **缓存（后续推荐）**：BOM 结构变化不频繁，可缓存级联结构（key: `bom_cascade:{product_id}`，BOM 更新时删除相关缓存），库存单独实时查询（或短 TTL 30 秒）
