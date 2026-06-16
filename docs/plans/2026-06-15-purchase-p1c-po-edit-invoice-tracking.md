# P1c: PO 确认后修改明细 + 发票状态追踪实现计划

> 日期：2026-06-15  
> 优先级：P1（核心功能补齐）  
> 参考：Odoo `purchase_order.py` write() + `qty_invoiced` / `invoice_status`；ERPNext `can_update_items()` + `per_billed`  
> 改动范围：migration + model + service 新方法 + arrival_handler 联动

---

## 一、设计参考

### Odoo 实现

- **PO 确认后修改明细**：`purchase.order.line` 的 `write()` 在 `state == 'purchase'` 时允许修改，并通过 `message_post_with_source` 追踪变更（quantity 变更自动通知）
- **发票状态追踪**：
  - 行级：`qty_invoiced`（已开票数量，从关联的 `account.move.line` 求和），`qty_to_invoice`（待开票数量 = qty_received - qty_invoiced）
  - 头级：`invoice_status`（no / to invoice / invoiced），通过 `_get_invoiced()` compute

### ERPNext 实现

- **PO 确认后修改明细**：`can_update_items()` 返回 True 时允许修改（未进入委外流程）
- **开票状态追踪**：
  - 头级：`per_billed`（已开票百分比），`advance_payment_status`
  - 状态联动：`To Receive and Bill` → `To Bill` → `Completed`

### ABT 设计决策

- 采用 **Odoo 模式**：允许 Confirmed 状态修改明细，通过审计日志记录变更
- 限制：仅允许 Confirmed 和 PartiallyReceived 状态修改（Received/Closed 不允许）
- 修改内容限制：追加行、修改数量、修改单价（不允许删除已收货行）
- 发票状态：新增 `qty_invoiced` 字段，在对账确认时更新

---

## 二、数据模型变更

### Migration: `049_purchase_po_edit_invoice_tracking.sql`

```sql
BEGIN;

-- ============================================================================
-- 1. PO 明细增加已开票数量字段
-- ============================================================================

ALTER TABLE purchase_order_items
    ADD COLUMN qty_invoiced  NUMERIC(18,6) NOT NULL DEFAULT 0,
    ADD COLUMN invoice_status SMALLINT     NOT NULL DEFAULT 1;
    -- InvoiceLineStatus: 1=NoInvoice, 2=ToInvoice, 3=Invoiced

-- ============================================================================
-- 2. PO 主表增加开票状态
-- ============================================================================

ALTER TABLE purchase_orders
    ADD COLUMN invoice_status SMALLINT NOT NULL DEFAULT 1,
    ADD COLUMN per_billed     NUMERIC(5,2) NOT NULL DEFAULT 0;
    -- 1=NoInvoice, 2=ToInvoice, 3=FullyInvoiced
    -- per_billed: 已开票金额百分比

-- ============================================================================
-- 3. 新增发票行状态枚举（应用层定义）
--    InvoiceLineStatus: NoInvoice(1), ToInvoice(2), Invoiced(3)
-- ============================================================================

-- 状态机定义：PurchaseOrder 增加发票状态关联记录
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PurchaseOrderItem', 'NoInvoice',  '未开票',   TRUE,  FALSE),
    ('PurchaseOrderItem', 'ToInvoice',  '待开票',   FALSE, FALSE),
    ('PurchaseOrderItem', 'Invoiced',   '已开票',   FALSE, TRUE);

COMMIT;
```

---

## 三、PO 确认后修改明细

### 3.1 Service Trait 新增方法

**文件**：`abt-core/src/purchase/order/service.rs`

```rust
#[async_trait]
pub trait PurchaseOrderService: Send + Sync {
    // ... 已有方法 ...

    /// 确认后修改明细（追加/修改行，不允许删除已收货行）
    /// 仅 Confirmed / PartiallyReceived 状态可调用
    async fn update_items_after_confirm(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        item_changes: Vec<PoItemChange>,
        idempotency_key: Option<String>,
    ) -> Result<()>;
}

/// 明细变更指令
#[derive(Debug, Clone)]
pub enum PoItemChange {
    /// 追加新行
    AddItem(CreateOrderItemRequest),
    /// 修改已有行（数量、单价、折扣、税率）
    UpdateItem {
        item_id: i64,
        quantity: Option<Decimal>,
        unit_price: Option<Decimal>,
        discount_pct: Option<Decimal>,
        tax_rate_id: Option<Option<i64>>, // Some(None) = 清除税率
    },
    /// 删除行（仅允许未收货的行）
    RemoveItem { item_id: i64 },
}
```

### 3.2 实现逻辑

**文件**：`abt-core/src/purchase/order/implt.rs`

```rust
async fn update_items_after_confirm(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    order_id: i64,
    item_changes: Vec<PoItemChange>,
    idempotency_key: Option<String>,
) -> Result<()> {
    // 幂等检查
    if let Some(ref key) = idempotency_key { ... }

    // 1. 校验 PO 状态
    let order = PurchaseOrderRepo::get_by_id(&mut *db, order_id).await?
        .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;
    
    if !matches!(order.status, PurchaseOrderStatus::Confirmed | PurchaseOrderStatus::PartiallyReceived) {
        return Err(DomainError::business_rule(
            "仅 Confirmed/PartiallyReceived 状态的订单可以修改明细"
        ));
    }

    let existing_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, order_id).await?;
    let mut audit_changes = Vec::new();

    // 2. 逐个处理变更
    for change in &item_changes {
        match change {
            PoItemChange::AddItem(new_item) => {
                // 校验
                if new_item.quantity <= Decimal::ZERO || new_item.unit_price <= Decimal::ZERO {
                    return Err(DomainError::validation("追加行的数量和单价必须大于 0"));
                }
                // insert
                let max_line_no = existing_items.iter().map(|i| i.line_no).max().unwrap_or(0);
                PurchaseOrderItemRepo::insert_single(&mut *db, order_id, max_line_no + 1, new_item).await?;
                audit_changes.push(json!({"action": "add", "product_id": new_item.product_id}));
            }
            PoItemChange::UpdateItem { item_id, quantity, unit_price, discount_pct, tax_rate_id } => {
                let item = existing_items.iter().find(|i| i.id == *item_id)
                    .ok_or_else(|| DomainError::not_found("PurchaseOrderItem"))?;
                
                // 校验：修改后数量不能小于已收货数量
                if let Some(new_qty) = quantity {
                    if *new_qty < item.received_qty {
                        return Err(DomainError::validation(format!(
                            "修改后数量 {} 不能小于已收货数量 {}", new_qty, item.received_qty
                        )));
                    }
                }
                
                // 执行 update
                PurchaseOrderItemRepo::update_fields_after_confirm(
                    &mut *db, *item_id, *quantity, *unit_price, *discount_pct, *tax_rate_id
                ).await?;
                
                audit_changes.push(json!({
                    "action": "update", "item_id": item_id,
                    "quantity": quantity, "unit_price": unit_price
                }));
            }
            PoItemChange::RemoveItem { item_id } => {
                let item = existing_items.iter().find(|i| i.id == *item_id)
                    .ok_or_else(|| DomainError::not_found("PurchaseOrderItem"))?;
                
                // 校验：只有未收货的行可以删除
                if item.received_qty > Decimal::ZERO {
                    return Err(DomainError::business_rule(format!(
                        "行 {} 已有收货记录，不能删除", item.line_no
                    )));
                }
                
                PurchaseOrderItemRepo::delete_by_id(&mut *db, *item_id).await?;
                audit_changes.push(json!({"action": "remove", "item_id": item_id}));
            }
        }
    }

    // 3. 重算总金额
    let updated_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, order_id).await?;
    let total_amount: Decimal = updated_items.iter()
        .map(|i| i.quantity * i.unit_price * (Decimal::ONE - i.discount_pct / Decimal::from(100)))
        .sum();
    PurchaseOrderRepo::update_total_amount(&mut *db, order_id, total_amount).await?;

    // 4. 审计日志（记录所有变更）
    new_audit_log_service(self.pool.clone())
        .record(ctx, db, RecordAuditLogReq {
            entity_type: ENTITY_TYPE,
            entity_id: order_id,
            action: AuditAction::Update,
            changes: Some(json!({ "item_changes": audit_changes })),
            context: None,
        }).await?;

    // 5. 发布事件
    new_domain_event_bus(self.pool.clone())
        .publish(ctx, db, EventPublishRequest {
            event_type: DomainEventType::PurchaseOrderModified,
            aggregate_type: ENTITY_TYPE.to_string(),
            aggregate_id: order_id,
            payload: json!({}),
            idempotency_key: None,
        }).await?;

    Ok(())
}
```

### 3.3 Repo 新增方法

**文件**：`abt-core/src/purchase/order/repo.rs`

```rust
impl PurchaseOrderItemRepo {
    // ... 已有方法 ...

    /// 插入单行（追加模式）
    pub async fn insert_single(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
        line_no: i32,
        item: &CreateOrderItemRequest,
    ) -> Result<()> { ... }

    /// 确认后更新行字段
    pub async fn update_fields_after_confirm(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        quantity: Option<Decimal>,
        unit_price: Option<Decimal>,
        discount_pct: Option<Decimal>,
        tax_rate_id: Option<Option<i64>>,
    ) -> Result<()> {
        // 动态构建 UPDATE SET 子句
        ...
    }

    /// 按主键删除行
    pub async fn delete_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
    ) -> Result<()> {
        sqlx::query("DELETE FROM purchase_order_items WHERE id = $1")
            .bind(item_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }
}
```

---

## 四、发票状态追踪

### 4.1 发票状态枚举

**文件**：`abt-core/src/purchase/enums.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[repr(i16)]
pub enum InvoiceStatus {
    NoInvoice = 1,     // 未开票
    ToInvoice = 2,     // 待开票
    FullyInvoiced = 3, // 已开票
}
```

### 4.2 PO Model 增加字段

```rust
pub struct PurchaseOrder {
    // ... 已有字段 ...
    pub invoice_status: InvoiceStatus,
    pub per_billed: Decimal,         // 已开票金额百分比
}

pub struct PurchaseOrderItem {
    // ... 已有字段 ...
    pub qty_invoiced: Decimal,
    pub invoice_status: InvoiceStatus,
}
```

### 4.3 对账确认时更新开票状态

**文件**：`abt-core/src/purchase/reconciliation/implt.rs` 的 `confirm()` 方法中

在对账单确认时，更新关联 PO 明细的 `qty_invoiced`：

```rust
// reconciliation confirm() 中，状态转换成功后

// 1. 获取对账明细
let recon_items = PurchaseReconItemRepo::list_by_reconciliation_id(&mut *db, id).await?;

// 2. 逐行更新 PO item 的 qty_invoiced
for item in &recon_items {
    if item.confirmed {
        PurchaseOrderItemRepo::add_qty_invoiced(
            &mut *db, item.order_item_id, item.received_qty
        ).await?;
    }
}

// 3. 重算各 PO 的 invoice_status 和 per_billed
let affected_po_ids: HashSet<i64> = recon_items.iter().map(|i| i.order_id).collect();
for po_id in affected_po_ids {
    self.recompute_invoice_status(&ctx, &mut *db, po_id).await?;
}
```

### 4.4 重算开票状态

```rust
async fn recompute_invoice_status(
    &self,
    ctx: &ServiceContext,
    db: &mut sqlx::postgres::PgConnection,
    po_id: i64,
) -> Result<()> {
    let items = PurchaseOrderItemRepo::list_by_order_id(db, po_id).await?;
    
    // 计算行级状态
    for item in &items {
        let line_status = if item.qty_invoiced >= item.received_qty && item.qty_invoiced > Decimal::ZERO {
            InvoiceStatus::FullyInvoiced
        } else if item.qty_invoiced > Decimal::ZERO {
            InvoiceStatus::ToInvoice
        } else {
            InvoiceStatus::NoInvoice
        };
        PurchaseOrderItemRepo::update_invoice_status(db, item.id, line_status).await?;
    }
    
    // 计算头级状态
    let all_invoiced = items.iter().all(|i| i.invoice_status == InvoiceStatus::FullyInvoiced);
    let any_invoiced = items.iter().any(|i| i.invoice_status != InvoiceStatus::NoInvoice);
    
    let po_status = if all_invoiced {
        InvoiceStatus::FullyInvoiced
    } else if any_invoiced {
        InvoiceStatus::ToInvoice
    } else {
        InvoiceStatus::NoInvoice
    };
    
    // 计算 per_billed
    let total_amount: Decimal = items.iter().map(|i| i.price_total).sum();
    let invoiced_amount: Decimal = items.iter()
        .map(|i| i.qty_invoiced * i.unit_price).sum();
    let per_billed = if total_amount > Decimal::ZERO {
        invoiced_amount / total_amount * Decimal::from(100)
    } else {
        Decimal::ZERO
    };
    
    PurchaseOrderRepo::update_invoice_status(db, po_id, po_status, per_billed).await?;
    Ok(())
}
```

### 4.5 Repo 新增方法

```rust
impl PurchaseOrderItemRepo {
    /// 累加已开票数量
    pub async fn add_qty_invoiced(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        qty: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_order_items SET qty_invoiced = qty_invoiced + $2 WHERE id = $1"
        )
        .bind(item_id)
        .bind(qty)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 更新行开票状态
    pub async fn update_invoice_status(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        status: InvoiceStatus,
    ) -> Result<()> { ... }
}

impl PurchaseOrderRepo {
    /// 更新头级开票状态和百分比
    pub async fn update_invoice_status(
        executor: &mut sqlx::postgres::PgConnection,
        po_id: i64,
        status: InvoiceStatus,
        per_billed: Decimal,
    ) -> Result<()> { ... }
}
```

---

## 五、Web 层变更

### 5.1 PO 详情页

- Confirmed/PartiallyReceived 状态显示"修改明细"按钮
- 修改明细弹窗：支持追加行、修改行数量/单价、删除未收货行
- 已收货行标灰，不可删除，数量不可低于已收货量
- 底部增加发票状态徽章（未开票/待开票/已开票）+ 开票百分比进度条

### 5.2 PO 列表页

- 增加"开票状态"列筛选

---

## 六、验收标准

1. Confirmed PO 可追加新行，审计日志记录追加操作
2. Confirmed PO 可修改行数量，但修改后数量不低于已收货量
3. 已收货行不可删除，未收货行可删除
4. PartiallyReceived 状态同样支持修改
5. Received/Closed 状态不可修改
6. 对账确认后，关联 PO 明细的 qty_invoiced 正确累加
7. PO 头级 invoice_status 和 per_billed 正确计算
8. `cargo clippy` 通过

---

## 七、实施步骤

| 步骤 | 内容 | 工作量 |
|---|---|---|
| 1 | Migration `049_purchase_po_edit_invoice_tracking.sql` | 0.5h |
| 2 | 新增 `InvoiceStatus` 枚举 + `PoItemChange` 类型 | 0.5h |
| 3 | 修改 `order/model.rs` 增加字段 | 0.5h |
| 4 | 新增 `update_items_after_confirm` service 方法 | 2h |
| 5 | Repo 新增 `insert_single` / `update_fields_after_confirm` / `delete_by_id` | 1.5h |
| 6 | 对账确认联动更新 qty_invoiced | 1.5h |
| 7 | `recompute_invoice_status` 实现 | 1h |
| 8 | Web 页面：修改明细弹窗 | 2h |
| 9 | Web 页面：发票状态展示 | 1h |
| 10 | 新增事件 `PurchaseOrderModified` | 0.5h |
| 11 | 更新设计文档 | 1h |
| **合计** | | **12h** |
