# P0: 采购模块 Bug 修复实现计划

> 日期：2026-06-15  
> 优先级：P0（立即修复）  
> 改动范围：5 个文件的逻辑修复，不改数据模型，不需要 migration  
> 验证方式：`cargo clippy -p abt-core` + `cargo test -p abt-core`

---

## BUG-001/002: create() + update() 校验前置

### 问题

`PurchaseOrderServiceImpl::create()` 在 `implt.rs:88-117` 先 INSERT 主表+明细，然后才校验 quantity/unit_price。
`PurchaseOrderServiceImpl::update()` 在 `implt.rs:460-481` 先 DELETE 旧明细+INSERT 新明细，然后才校验。

### 修复方案

**文件**：`abt-core/src/purchase/order/implt.rs`

**create() 改动**：将第 105-117 行的校验块移到第 84 行（计算 total_amount）之后、第 88 行（INSERT 主表）之前。

```rust
// 2. 计算总金额
let total_amount: Decimal = req.items.iter().map(|i| i.quantity * i.unit_price).sum();

// 2.5 校验明细（前置！）
for (i, item) in req.items.iter().enumerate() {
    if item.quantity <= Decimal::ZERO {
        return Err(DomainError::validation(
            format!("订单明细第 {} 行数量必须大于 0", i + 1)
        ));
    }
    if item.unit_price <= Decimal::ZERO {
        return Err(DomainError::validation(
            format!("订单明细第 {} 行单价必须大于 0", i + 1)
        ));
    }
}

// 3. 插入主表
let id = PurchaseOrderRepo::insert(...)...
```

**update() 改动**：将第 469-481 行的校验块移到第 460 行（update_fields）之前。

```rust
// 0.5 校验明细（前置！）
for (i, item) in items.iter().enumerate() {
    if item.quantity <= Decimal::ZERO {
        return Err(DomainError::validation(
            format!("订单明细第 {} 行数量必须大于 0", i + 1)
        ));
    }
    if item.unit_price <= Decimal::ZERO {
        return Err(DomainError::validation(
            format!("订单明细第 {} 行单价必须大于 0", i + 1)
        ));
    }
}

// 1. 更新订单头
PurchaseOrderRepo::update_fields(&mut *db, id, &req).await?;
```

### 验收标准

- `create()` 传入 quantity=0 时，返回 `Validation` 错误，数据库无新记录
- `create()` 传入 unit_price=0 时，返回 `Validation` 错误，数据库无新记录
- `update()` 同上，且旧明细不被删除

---

## BUG-003: create_from_quotation() 增加校验 + 修复数量默认值

### 问题

`implt.rs:131-252`：完全不做 quantity/unit_price 校验；第 170 行 `quantity: qi.min_order_qty.unwrap_or(Decimal::ONE)` 把 min_order_qty 当作数量使用。

### 修复方案

**文件**：`abt-core/src/purchase/order/implt.rs`

1. 第 170 行改为使用报价明细的实际数量（需确认 quotation_items 是否有 quantity 字段——当前 schema 中 quotation_items 没有 quantity 字段，只有 `min_order_qty`）。

   **决策**：既然 quotation_items 的语义是"供应商对该产品的报价"，min_order_qty 就是"最小起订量"。创建 PO 时应使用 min_order_qty 作为初始数量（如果没有则必须由用户在 UI 上指定，不应默认为 1）。

   修复：将默认值改为返回校验错误而非默认 1：

```rust
let quantity = qi.min_order_qty.ok_or_else(|| DomainError::validation(
    format!("报价明细第 {} 行未设置最小起订量，无法自动创建订单", idx + 1)
))?;
```

2. 增加 unit_price > 0 校验：

```rust
if qi.unit_price <= Decimal::ZERO {
    return Err(DomainError::validation(
        format!("报价明细第 {} 行单价必须大于 0", idx + 1)
    ));
}
```

### 验收标准

- 从无 min_order_qty 的报价单创建 PO 时，返回 Validation 错误
- 从 unit_price=0 的报价单创建 PO 时，返回 Validation 错误

---

## BUG-004: 状态机 `.ok()` 吞错误（3 处）

### 问题

以下 3 个文件的 `create()` 方法中，状态机初始转换用 `.ok()` 吞掉错误：
- `abt-core/src/purchase/payment/implt.rs:123-126`
- `abt-core/src/purchase/reconciliation/implt.rs:125-128`
- `abt-core/src/purchase/return_order/implt.rs:133-136`

### 修复方案

**文件**（3 个），统一将 `.ok()` 改为 `?`：

```rust
// 修改前
new_state_machine_service(self.pool.clone())
    .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
    .await
    .ok();

// 修改后
new_state_machine_service(self.pool.clone())
    .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
    .await?;
```

### 验收标准

- `cargo clippy` 无警告
- 状态机转换失败时，create() 返回错误而非静默成功

---

## BUG-005: 退货单增加退货数量上限校验

### 问题

`return_order/implt.rs:43-139`：`create()` 不校验 returned_qty ≤ (received_qty - 已退数量)。

### 修复方案

**文件**：`abt-core/src/purchase/return_order/implt.rs`

在 `create()` 方法中，第 71 行（订单状态校验通过后）增加明细级数量校验：

```rust
// 2. 获取 PO 明细，校验退货数量
let po_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, req.order_id)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

for item in &req.items {
    let po_item = po_items.iter().find(|p| p.id == item.order_item_id)
        .ok_or_else(|| DomainError::validation(format!(
            "退货明细关联的订单行 {} 不存在", item.order_item_id
        )))?;

    let max_returnable = po_item.received_qty - po_item.returned_qty;
    if item.returned_qty > max_returnable {
        return Err(DomainError::validation(format!(
            "退货数量 {} 超过可退数量 {}（已收 {} - 已退 {}）",
            item.returned_qty, max_returnable, po_item.received_qty, po_item.returned_qty
        )));
    }
}
```

**需要增加 import**：
```rust
use crate::purchase::order::repo::PurchaseOrderItemRepo;
```

### 验收标准

- 退回数量超过 (received_qty - returned_qty) 时，返回 Validation 错误
- 退回数量等于可退数量时，正常创建

---

## BUG-006: 对账单按期间过滤 + 防重复

### 问题

`reconciliation/implt.rs:61-68`：`list_received_by_supplier(supplier_id)` 查询全部已收货明细，不按期间过滤，不排除已在其他对账单中的明细。

### 修复方案

**文件**：`abt-core/src/purchase/order/repo.rs` + `abt-core/src/purchase/reconciliation/implt.rs`

#### Step 1: 修改 repo 方法签名和 SQL

`PurchaseOrderItemRepo::list_received_by_supplier` 增加参数：

```rust
/// 按供应商查询已收货且未关联到已确认对账单的订单明细
pub async fn list_unreconciled_received_by_supplier(
    executor: &mut sqlx::postgres::PgConnection,
    supplier_id: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> Result<Vec<PurchaseOrderItem>> {
    sqlx::query_as::<_, PurchaseOrderItem>(
        r#"
        SELECT poi.id, poi.order_id, poi.line_no, poi.product_id, poi.description,
               poi.quantity, poi.unit_price, poi.amount, poi.received_qty,
               poi.inspected_qty, poi.returned_qty, poi.quotation_item_id,
               poi.expected_delivery_date
        FROM purchase_order_items poi
        JOIN purchase_orders po ON po.id = poi.order_id
        WHERE po.supplier_id = $1
          AND po.status IN ($2, $3, $4)
          AND po.deleted_at IS NULL
          AND poi.received_qty > 0
          AND po.order_date BETWEEN $5 AND $6
          AND NOT EXISTS (
              SELECT 1 FROM purchase_recon_items pri
              JOIN purchase_reconciliations pr ON pr.id = pri.reconciliation_id
              WHERE pri.order_item_id = poi.id
                AND pr.status >= 2  -- Confirmed or Settled
                AND pr.deleted_at IS NULL
          )
        ORDER BY po.order_date, poi.line_no
        "#,
    )
    .bind(supplier_id)
    .bind(PurchaseOrderStatus::Confirmed)
    .bind(PurchaseOrderStatus::PartiallyReceived)
    .bind(PurchaseOrderStatus::Received)
    .bind(period_start)
    .bind(period_end)
    .fetch_all(executor)
    .await.map_err(Into::into)
}
```

#### Step 2: 修改 reconciliation create() 调用

`reconciliation/implt.rs:61-68` 改为：

```rust
// 2. 查询该供应商当期未对账的已收货订单明细
let order_items = PurchaseOrderItemRepo::list_unreconciled_received_by_supplier(
    &mut *db,
    supplier_id,
    period_start,
    period_end,
)
.await
.map_err(|e| DomainError::Internal(e.into()))?;
```

需要在 `create()` 方法签名中增加 `period_start: NaiveDate, period_end: NaiveDate` 参数，或者在 service trait 的 `create` 方法中解析 period 字符串得到起止日期。

#### Step 3: Service trait 修改

`reconciliation/service.rs` 的 `create` 方法签名修改为：

```rust
async fn create(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    supplier_id: i64,
    period: String,         // 如 "2026-06"
    period_start: NaiveDate, // 2026-06-01
    period_end: NaiveDate,   // 2026-06-30
    idempotency_key: Option<String>,
) -> Result<i64>;
```

### 验收标准

- 同一订单明细不会被拉入两张已确认的对账单
- 对账范围受 period_start/period_end 限制
- 查询结果只包含 received_qty > 0 的明细

---

## 实施顺序

1. BUG-001/002（校验前置）→ `cargo clippy` 验证
2. BUG-003（quotation 校验）→ `cargo clippy` 验证
3. BUG-004（.ok() 吞错误）→ `cargo clippy` 验证
4. BUG-005（退货数量校验）→ `cargo clippy` 验证
5. BUG-006（对账过滤）→ 需要 repo + service + route 同步修改 → `cargo clippy` 验证

## 预计工作量

| Bug | 工作量 |
|---|---|
| BUG-001/002 | 0.5h |
| BUG-003 | 0.5h |
| BUG-004 | 0.5h |
| BUG-005 | 1h |
| BUG-006 | 2h |
| **合计** | **4.5h** |
