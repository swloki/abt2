# 级联查询库存功能设计

## 概述

给定一个产品 ID 或 product_code，查出该产品被哪些 BOM 引用，并按 BOM 分别展示该产品节点的直接子节点及其库存总量。

## 方案选择

采用两次查询方案：
1. 查询产品被哪些 BOM 引用，以及在这些 BOM 中该产品节点的直接子节点
2. 批量查询子节点产品的库存汇总

避免单次大 JOIN（复杂度高）和 CTE 递归（当前只需直接子节点，过度设计）。

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
  double quantity = 6;
  string total_stock = 7;
  double loss_rate = 8;
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

1. **解析入参**：根据 `product_id` 或 `product_code` 查出产品信息，得到确定的 `product_id`
2. **第一次查询**：找到包含该产品的 bom_node，再找这些节点的直接子节点

```sql
SELECT
  bn_parent.bom_id,
  b.bom_name,
  child.id AS node_id,
  child.product_id,
  child.product_code,
  p.pdt_name AS product_name,
  child.unit,
  child.quantity,
  child.loss_rate
FROM bom_nodes bn_parent
JOIN bom b ON b.bom_id = bn_parent.bom_id
JOIN bom_nodes child ON child.parent_id = bn_parent.id AND child.bom_id = bn_parent.bom_id
JOIN products p ON p.product_id = child.product_id
WHERE bn_parent.product_id = $1
  AND b.deleted_at IS NULL
  AND p.deleted_at IS NULL
ORDER BY b.bom_id, child."order"
```

3. **第二次查询**：批量获取子节点产品的库存汇总

```sql
SELECT
  i.product_id,
  SUM(i.quantity) AS total_stock
FROM inventory i
WHERE i.product_id = ANY($1)
GROUP BY i.product_id
```

4. **组装结果**：将库存汇总 map 合并到子节点数据中，按 `bom_id` 分组构建响应

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
- 接收 `product_id` 或 `product_code`，如果传的是 code 则先查产品表拿到 id
- 调用 repo 的两次查询
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
