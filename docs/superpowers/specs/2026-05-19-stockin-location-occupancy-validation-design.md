---
name: stockin-location-occupancy-validation
description: 入库时校验库位占用，一个库位同一时间只能存放一个产品
---

# 入库库位占用校验

## 规则

一个库位同一时间只能存放一个产品。除非该库位当前产品的库存数量为 0。

| 场景 | 结果 |
|------|------|
| 库位无任何库存记录 | 允许入库 |
| 库位有同一产品的库存（quantity > 0） | 允许入库（累加） |
| 库位有同一产品的库存（quantity = 0） | 允许入库 |
| 库位有其他产品的库存（quantity > 0） | 拒绝，返回错误 |
| 库位有其他产品的库存（quantity = 0） | 允许入库 |

## 影响范围

仅 `StockIn` 接口需要修改。`StockOut` 和 `Adjust` 不需要（只操作已有产品库存）。

## 实现方案

### 1. 新增 repository 方法

在 `abt/src/repositories/inventory_repo.rs` 中新增：

```rust
/// 查询库位上是否有其他产品的库存（quantity > 0）
/// 返回占用该库位的产品ID和数量（如果有的话）
pub async fn find_occupant_by_location(
    executor: PgExecutor<'_>,
    location_id: i64,
) -> Result<Option<(i64, Decimal)>>
```

SQL：
```sql
SELECT product_id, quantity FROM inventory
WHERE location_id = $1 AND quantity > 0
LIMIT 1
```

### 2. 修改 stock_in 方法

在 `abt/src/implt/inventory_service_impl.rs` 的 `stock_in` 中，在 `get_or_create_for_update` 之前增加校验：

1. 调用 `InventoryRepo::find_occupant_by_location(executor, req.location_id)`
2. 如果返回 `Some((occupant_product_id, qty))` 且 `occupant_product_id != req.product_id`
3. 查询占用产品名称：`ProductRepo::find_by_id(&self.pool, occupant_product_id)`
4. 返回错误：`"库位已被产品 {product_name} 占用（库存: {quantity}），请先清空该库位"`

### 3. 错误响应

使用 `tonic::Status::already_exists()` 返回 gRPC AlreadyExists 状态码。

## 不做的事

- 不修改 `StockOut` / `Adjust` 接口
- 不修改前端 `LocationSelect` 组件
- 不修改已有的 `get_location_occupants` 方法（导入流程继续使用）
