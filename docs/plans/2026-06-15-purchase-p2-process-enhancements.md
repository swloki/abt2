# P2: 采购流程增强实现计划

> 日期：2026-06-15  
> 优先级：P2（流程级改进）  
> 参考：Odoo `purchase_order.py` button_approve/button_confirm + `product.supplierinfo` + `action_merge`；ERPNext `Authorization Control` + `Supplier Scorecard`  
> 改动范围：跨模块，新建 2 个模块 + 修改多个现有模块

---

## 一、多阶段审批（参考 Odoo + ERPNext）

### 1.1 设计参考

**Odoo**：
- PO 状态机增加 `to approve` 状态
- `button_confirm()` 中检查 `_approval_allowed()`：若金额超过阈值则进入 `to approve`，否则直接 `button_approve()`
- 双重审批：采购员提交 → 采购经理审批

**ERPNext**：
- `Authorization Control` 独立 DocType，按金额段配置审批层级
- PO submit 时自动检查金额是否需要审批

### 1.2 ABT 设计

**状态机扩展**：PurchaseOrder 增加 `PendingApproval` 状态

```
Draft → PendingApproval → Confirmed → PartiallyReceived → Received → Closed
Draft → Cancelled
PendingApproval → Draft (退回修改)
PendingApproval → Cancelled
```

**审批配置表**：

```sql
-- Migration: 050_purchase_approval_config.sql

CREATE TABLE purchase_approval_rules (
    id              BIGSERIAL      PRIMARY KEY,
    name            VARCHAR(64)    NOT NULL,
    min_amount      NUMERIC(20,4)  NOT NULL DEFAULT 0,   -- 金额下限
    max_amount      NUMERIC(20,4),                       -- 金额上限（NULL = 无上限）
    approver_role   VARCHAR(64)    NOT NULL,             -- 审批角色标识
    approver_id     BIGINT,                              -- 指定审批人（可选）
    is_active       BOOLEAN        NOT NULL DEFAULT TRUE,
    sort_order      INTEGER        NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_par_active_amount ON purchase_approval_rules (is_active, min_amount)
    WHERE deleted_at IS NULL;

-- 状态机扩展
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PurchaseOrder', 'PendingApproval', '待审批', FALSE, FALSE);

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PurchaseOrder', 'Draft',           'PendingApproval', NULL, 1.5),
    ('PurchaseOrder', 'PendingApproval', 'Confirmed',       NULL, 1.6),
    ('PurchaseOrder', 'PendingApproval', 'Draft',           NULL, 1.7),
    ('PurchaseOrder', 'PendingApproval', 'Cancelled',       NULL, 1.8);
```

**PurchaseOrderStatus 枚举新增**：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[repr(i16)]
pub enum PurchaseOrderStatus {
    Draft = 1,
    PendingApproval = 2,  // 新增
    Confirmed = 3,
    PartiallyReceived = 4,
    Received = 5,
    Closed = 6,
    Cancelled = 7,
}
```

### 1.3 审批流程

**PO 提交逻辑**（修改 `order/implt.rs`）：

将 `confirm()` 拆分为 `submit()`（提交审批）和 `approve()`（审批通过）：

```rust
/// 提交 PO（自动判断是否需要审批）
async fn submit(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
    idempotency_key: Option<String>,
) -> Result<()> {
    // ... 已有校验逻辑 ...

    // 查找匹配的审批规则
    let approval_rule = PurchaseApprovalRuleRepo::find_by_amount(&mut *db, order.amount_total).await?;

    if let Some(rule) = approval_rule {
        // 需要审批 → 进入 PendingApproval
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "PendingApproval", None).await?;
        PurchaseOrderRepo::update_status(&mut *db, id, PurchaseOrderStatus::PendingApproval, &order.updated_at).await?;
        
        // 发布事件：PO 待审批（触发通知）
        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::PurchaseOrderPendingApproval,
                aggregate_type: ENTITY_TYPE.to_string(),
                aggregate_id: id,
                payload: json!({ "approver_role": rule.approver_role, "amount": order.amount_total }),
                idempotency_key: None,
            }).await?;
    } else {
        // 无需审批 → 直接确认
        self.approve(ctx, db, id, idempotency_key).await?;
    }
    Ok(())
}

/// 审批通过
async fn approve(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
    idempotency_key: Option<String>,
) -> Result<()> {
    // ... 状态转换 PendingApproval → Confirmed ...
    // ... 发布事件 PurchaseOrderConfirmed ...
}

/// 退回修改（审批拒绝）
async fn reject(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
    reason: String,
    idempotency_key: Option<String>,
) -> Result<()> {
    // ... 状态转换 PendingApproval → Draft ...
    // ... 审计日志记录拒绝原因 ...
}
```

### 1.4 新建模块

```
abt-core/src/purchase/approval/
├── mod.rs       // factory: new_approval_service(pool)
├── service.rs   // trait PurchaseApprovalService: list_rules, create_rule, update_rule
├── model.rs     // PurchaseApprovalRule entity
└── repo.rs      // PurchaseApprovalRuleRepo: find_by_amount(), CRUD
```

---

## 二、供应商价格目录（参考 Odoo `product.supplierinfo`）

### 2.1 设计参考

**Odoo** `product.supplierinfo`：
- `partner_id`（供应商）
- `product_id` / `product_tmpl_id`（产品）
- `min_qty`（最小起订量）
- `price`（单价）
- `currency_id`
- `delay`（交货周期天数）
- `discount`（折扣）
- `date_start` / `date_end`（有效期）
- `sequence`（优先级）
- `product_code`（供应商产品编码）
- `product_name`（供应商产品名称）

**ERPNext** `Item Supplier`：
- `supplier`（供应商）
- `parent`（产品）
- `supplier_part_no`（供应商零件号）

### 2.2 ABT 设计

**新建表 `supplier_product_prices`**：

```sql
-- Migration: 051_supplier_product_prices.sql

CREATE TABLE supplier_product_prices (
    id                  BIGSERIAL      PRIMARY KEY,
    supplier_id         BIGINT         NOT NULL,
    product_id          BIGINT         NOT NULL,
    supplier_item_code  VARCHAR(64),              -- 供应商产品编码
    supplier_item_name  VARCHAR(256),             -- 供应商产品名称
    min_order_qty       NUMERIC(18,6)  NOT NULL DEFAULT 1,  -- 最小起订量
    price               NUMERIC(18,6)  NOT NULL,            -- 单价
    currency_code       VARCHAR(3)     NOT NULL DEFAULT 'CNY',
    discount_pct        NUMERIC(5,2)   NOT NULL DEFAULT 0,  -- 默认折扣
    lead_time_days      INTEGER        NOT NULL DEFAULT 0,  -- 交货周期
    tax_rate_id         BIGINT,                             -- 默认税率
    valid_from          DATE,                               -- 有效期起
    valid_until         DATE,                               -- 有效期止
    sequence            INTEGER        NOT NULL DEFAULT 10, -- 优先级
    is_active           BOOLEAN        NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ
);

CREATE INDEX idx_spp_supplier_product ON supplier_product_prices (supplier_id, product_id)
    WHERE deleted_at IS NULL AND is_active = TRUE;
CREATE INDEX idx_spp_product ON supplier_product_prices (product_id)
    WHERE deleted_at IS NULL AND is_active = TRUE;
```

### 2.3 新建模块

```
abt-core/src/purchase/supplier_price/
├── mod.rs       // factory: new_supplier_price_service(pool)
├── service.rs   // trait SupplierPriceService
├── model.rs     // SupplierProductPrice entity
└── repo.rs      // SupplierProductPriceRepo
```

### 2.4 Service Trait

```rust
#[async_trait]
pub trait SupplierPriceService: Send + Sync {
    /// 创建供应商-产品价格关联
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateSupplierPriceRequest) -> Result<i64>;
    
    /// 查询某供应商供应的所有产品
    async fn list_by_supplier(&self, ctx: &ServiceContext, db: PgExecutor<'_>, supplier_id: i64) -> Result<Vec<SupplierProductPrice>>;
    
    /// 查询某产品的所有供应商报价
    async fn list_by_product(&self, ctx: &ServiceContext, db: PgExecutor<'_>, product_id: i64) -> Result<Vec<SupplierProductPrice>>;
    
    /// 自动匹配最优价格（按优先级 + 有效期 + 起订量）
    /// 在 PO 创建时，选择产品+供应商后自动填充
    async fn match_best_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
        product_id: i64,
        quantity: Decimal,
    ) -> Result<Option<SupplierProductPrice>>;
    
    /// 获取上次采购价（参考 ERPNext get_last_purchase_rate）
    async fn get_last_purchase_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Option<(Decimal, NaiveDate)>>;

    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateSupplierPriceRequest) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
```

### 2.5 自动取价（参考 Odoo `_compute_price_unit_and_date_planned_and_name`）

PO 创建/编辑页面，选择产品后通过 HTMX 请求后端匹配价格：

```rust
// 路由: GET /admin/purchase/orders/match-price?supplier_id=X&product_id=Y&qty=Z
async fn match_price_handler(state, params) -> impl IntoResponse {
    let price = state.supplier_price_service()
        .match_best_price(&ctx, &db, params.supplier_id, params.product_id, params.qty).await?;
    
    // 返回 HTMX 片段：填充单价、折扣、税率、交货日期
    html! {
        @if let Some(p) = price {
            input value=(p.price) // ... 填充到表单
            "交货周期: " (p.lead_time_days) " 天"
        } @else {
            "无匹配的供应商价格"
        }
    }
}
```

### 2.6 确认 PO 时自动更新供应商价格目录（参考 Odoo `_add_supplier_to_product`）

```rust
// order/implt.rs confirm() 中
for item in &items {
    // 如果该供应商+产品没有价格记录，自动创建
    let existing = SupplierPriceRepo::find_exact(&mut *db, order.supplier_id, item.product_id).await?;
    if existing.is_none() {
        SupplierPriceRepo::insert(&mut *db, order.supplier_id, item.product_id, item.unit_price, ...).await?;
    }
}
```

---

## 三、合并 PO（参考 Odoo `action_merge`）

### 3.1 设计

将多个 Draft PO 按供应商合并为一个：

```rust
#[async_trait]
pub trait PurchaseOrderService {
    // ...
    
    /// 合并多个 Draft PO（必须是同一供应商）
    async fn merge_orders(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_ids: Vec<i64>,
        idempotency_key: Option<String>,
    ) -> Result<i64>;  // 返回合并后的 PO id
}
```

### 3.2 实现逻辑（参考 Odoo `action_merge`）

```rust
async fn merge_orders(&self, ctx, db, order_ids, idempotency_key) -> Result<i64> {
    // 1. 校验：所有 PO 都是 Draft 状态
    let orders = // 批量查询
    for o in &orders {
        if o.status != PurchaseOrderStatus::Draft {
            return Err(DomainError::business_rule("仅 Draft 状态的订单可以合并"));
        }
    }

    // 2. 校验：同一供应商
    let supplier_ids: HashSet<_> = orders.iter().map(|o| o.supplier_id).collect();
    if supplier_ids.len() != 1 {
        return Err(DomainError::business_rule("合并的订单必须属于同一供应商"));
    }

    // 3. 取最早的 PO 作为目标（保留编号）
    let target = orders.iter().min_by_key(|o| o.order_date).unwrap();

    // 4. 合并明细：相同 product + 相同 unit_price 的行合并数量
    let mut merged_items: HashMap<(i64, Decimal), CreateOrderItemRequest> = HashMap::new();
    let mut line_no = 1;
    
    for order in &orders {
        let items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, order.id).await?;
        for item in items {
            let key = (item.product_id, item.unit_price);
            merged_items.entry(key)
                .and_modify(|existing| {
                    existing.quantity += item.quantity;
                })
                .or_insert(CreateOrderItemRequest {
                    line_no,
                    product_id: item.product_id,
                    description: item.description.clone(),
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    discount_pct: item.discount_pct,
                    tax_rate_id: item.tax_rate_id,
                    quotation_item_id: item.quotation_item_id,
                    expected_delivery_date: item.expected_delivery_date,
                });
            line_no += 1;
        }
    }

    // 5. 更新目标 PO 的明细
    PurchaseOrderItemRepo::delete_by_order_id(&mut *db, target.id).await?;
    PurchaseOrderItemRepo::insert_items(&mut *db, target.id, &merged_items.into_values().collect::<Vec<_>>()).await?;

    // 6. 更新总金额
    let total = // 重算
    PurchaseOrderRepo::update_total_amount(&mut *db, target.id, total).await?;

    // 7. 取消其他 PO（非目标），并记录关联
    for order in &orders {
        if order.id != target.id {
            self.cancel(ctx, db, order.id, None).await?;
            new_document_link_service(self.pool.clone())
                .create_links(ctx, db, vec![LinkRequest {
                    source_type: DocumentType::PurchaseOrder,
                    source_id: target.id,
                    target_type: DocumentType::PurchaseOrder,
                    target_id: order.id,
                    link_type: LinkType::Merged,
                }]).await?;
        }
    }

    // 8. 审计日志
    new_audit_log_service(self.pool.clone())
        .record(ctx, db, RecordAuditLogReq {
            entity_type: ENTITY_TYPE,
            entity_id: target.id,
            action: AuditAction::Update,
            changes: Some(json!({"merged_from": order_ids})),
            context: None,
        }).await?;

    Ok(target.id)
}
```

---

## 四、三向匹配增强

### 4.1 当前状态

PaymentRequest 的 `create()` 仅校验金额容差（±0.5%），不匹配数量和明细行。

### 4.2 增强方案

在付款审批时，增加数量级匹配：

```rust
// payment/implt.rs approve() 中

// 三向匹配：PO 数量 ↔ 收货数量 ↔ 对账数量 ↔ 发票金额
if let Some(recon_id) = req.reconciliation_id {
    let recon = PurchaseReconciliationRepo::get_by_id(&mut *db, recon_id).await?
        .ok_or_else(|| DomainError::not_found("PurchaseReconciliation"))?;
    let recon_items = PurchaseReconItemRepo::list_by_reconciliation_id(&mut *db, recon_id).await?;

    for item in &recon_items {
        if !item.confirmed { continue; }
        
        let po_item = PurchaseOrderItemRepo::get_by_id(&mut *db, item.order_item_id).await?
            .ok_or_else(|| DomainError::not_found("PurchaseOrderItem"))?;
        
        // 校验：对账数量不能超过收货数量
        if item.received_qty > po_item.received_qty {
            return Err(DomainError::validation(format!(
                "对账明细行 {} 对账数量 {} 超过收货数量 {}",
                item.order_item_id, item.received_qty, po_item.received_qty
            )));
        }
        
        // 校验：退货后净额一致性
        let net_qty = item.received_qty - item.returned_qty;
        let expected_amount = net_qty * item.unit_price;
        if !within_tolerance(item.amount, expected_amount) {
            return Err(DomainError::validation(format!(
                "对账金额 {} 与净数量×单价 {} 不匹配",
                item.amount, expected_amount
            )));
        }
    }
}
```

---

## 五、验收标准

### 5.1 多阶段审批
1. 配置审批规则（金额 > 10000 需经理审批）
2. 创建金额 15000 的 PO，提交后进入 PendingApproval
3. 创建金额 5000 的 PO，提交后直接 Confirmed
4. PendingApproval 可退回 Draft（附拒绝原因）
5. PendingApproval 可审批通过到 Confirmed

### 5.2 供应商价格目录
1. 可录入供应商-产品价格关联（含起订量、交货周期、有效期）
2. PO 创建时选产品+供应商后自动匹配价格
3. 可查询某产品的所有供应商报价对比
4. 上次采购价可正确返回
5. PO 确认后自动创建缺失的供应商价格记录

### 5.3 合并 PO
1. 选择多个同供应商 Draft PO，合并为一个
2. 相同产品+单价的行自动合并数量
3. 被合并的 PO 自动取消，关联到目标 PO
4. 不同供应商的 PO 不允许合并

### 5.4 三向匹配
1. 付款审批时校验对账数量 ≤ 收货数量
2. 校验对账金额 = 净数量 × 单价（容差内）
3. 不匹配时返回明确的 Validation 错误

---

## 六、实施步骤

| 步骤 | 内容 | 工作量 |
|---|---|---|
| **多阶段审批** | | |
| 1 | Migration `050_purchase_approval_config.sql` | 0.5h |
| 2 | 枚举 + 状态机扩展 | 0.5h |
| 3 | 新建 `approval/` 模块 | 2h |
| 4 | 修改 `order/implt.rs` 拆分 submit/approve/reject | 2h |
| 5 | Web 页面：审批规则管理 + 审批操作 | 2h |
| **供应商价格目录** | | |
| 6 | Migration `051_supplier_product_prices.sql` | 0.5h |
| 7 | 新建 `supplier_price/` 模块 | 2h |
| 8 | PO 页面自动取价 HTMX 路由 | 1.5h |
| 9 | confirm 时自动更新价格目录 | 0.5h |
| 10 | Web 页面：供应商价格管理 | 2h |
| **合并 PO** | | |
| 11 | `merge_orders` service 方法 | 1.5h |
| 12 | Web 页面：合并操作 | 1h |
| **三向匹配** | | |
| 13 | payment approve 增强校验 | 1.5h |
| **设计文档** | | |
| 14 | 更新 `docs/uml-design/02-purchase.html` | 2h |
| **合计** | | **20h** |
