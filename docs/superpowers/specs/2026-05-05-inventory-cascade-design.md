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
  // BOM 中需要的数量，string 避免浮点精度问题，格式如 "1.500000"
  string quantity = 6;
  // 库存总量，string 保留 Decimal 精度，无库存时为 "0"
  string total_stock = 7;
  // 损耗率，string 避免浮点精度问题，格式如 "0.050000"
  string loss_rate = 8;
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
)
SELECT
  pp.product_id AS root_product_id,
  pp.product_code AS root_product_code,
  pp.pdt_name AS root_product_name,
  bn_parent.bom_id,
  b.bom_name,
  child.id AS node_id,
  child.product_id,
  child.product_code,
  p_child.pdt_name AS product_name,
  child.unit,
  child.quantity,
  child.loss_rate
FROM parent_product pp
JOIN bom_nodes bn_parent ON bn_parent.product_id = pp.product_id
JOIN bom b ON b.bom_id = bn_parent.bom_id
JOIN bom_nodes child ON child.parent_id = bn_parent.id
                     AND child.bom_id = bn_parent.bom_id
JOIN products p_child ON p_child.product_id = child.product_id
WHERE b.deleted_at IS NULL
  AND p_child.deleted_at IS NULL
ORDER BY b.bom_id, child."order";
```

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
   - Service 用 `HashMap<i64, Decimal>` 存库存汇总，合并到子节点
   - 按 `bom_id` 分组，组装 `CascadeInventoryResult`

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
    pub total_stock: Option<Decimal>,
    pub loss_rate: Decimal,
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
- 用 `HashMap<i64, Decimal>` 存库存汇总，合并到子节点数据
- 按 `bom_id` 分组，组装 `CascadeInventoryResult`

## 边界情况

| 场景 | 处理方式 |
|------|----------|
| 产品不存在 / 已软删除 | 返回 gRPC NOT_FOUND |
| 产品没有被任何 BOM 引用 | 返回空 bom_groups |
| 产品在 BOM 中是叶子节点（无子节点） | 该 BOM 的 children 为空数组 |
| 子节点产品无库存记录 | total_stock 返回 "0" |
| BOM 已软删除 | SQL 中 `b.deleted_at IS NULL` 过滤 |

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

- **结果集限制**：若产品被大量 BOM 引用（极端情况），考虑加 `LIMIT` 或分页参数，防止返回过大结果集
- **缓存（后续可选）**：BOM 结构变化不频繁，可缓存级联结构（key: `bom_cascade:{product_id}`），库存单独实时查询
