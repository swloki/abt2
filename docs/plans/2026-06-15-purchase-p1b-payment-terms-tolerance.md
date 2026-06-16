# P1b: 付款条款结构化 + 超收容差实现计划

> 日期：2026-06-15  
> 优先级：P1（核心功能补齐）  
> 参考：ERPNext `payment_schedule` 子表 + `Buying Settings`；Odoo `account.payment.term`  
> 改动范围：migration + 新建 payment_terms 模块 + 修改 arrival_handler + 新建 purchase_settings

---

## 一、设计参考

### ERPNext 实现

- **付款条款模板** `Payment Terms Template`：定义标准付款条款（如"30% 预付，70% 货到 30 天"）
- **付款计划子表** `Payment Schedule`：PO 子表，根据模板自动生成行
  - `due_date`（到期日）
  - `invoice_portion`（付款百分比，如 30.00 表示 30%）
  - `payment_amount`（计算金额）
  - `paid_amount`（已付金额）
  - `status`（待付/部分/已付）
- **采购设置** `Buying Settings`：
  - `over_order_allowance`（超下单容差 %）
  - `over_delivery_receipt_allowance`（超收货容差 %）
  - `maintain_same_rate`（全链路价格一致性）
  - `po_required`（收货是否必须关联 PO）
  - `pr_required`（发票是否必须关联收货单）

### Odoo 实现

- **付款条款** `account.payment.term`：定义付款行模板
  - `line_ids` → `account.payment.term.line`：每行 `value_amount`（百分比）、`nb_days`（到期天数）
- **供应商默认付款条款**：`res.partner.property_supplier_payment_term_id`
- **容差**：没有全局配置，基于行级 `product_qty` vs `qty_received` 比较

### ABT 设计决策

- 采用 **ERPNext 模式**：PO 子表 `payment_schedule`（结构化分期）
- 新建 **purchase_settings** 单行配置表（参考 ERPNext `Buying Settings`）
- 付款条款模板可选实施（P1 先实现 PO 子表，模板可后续 P2 补充）

---

## 二、数据模型变更

### Migration: `048_purchase_payment_terms_tolerance.sql`

```sql
BEGIN;

-- ============================================================================
-- 1. 付款计划子表（PO 关联子表）
-- ============================================================================

CREATE TABLE purchase_payment_schedules (
    id              BIGSERIAL      PRIMARY KEY,
    order_id        BIGINT         NOT NULL,
    line_no         INTEGER        NOT NULL,
    due_date        DATE           NOT NULL,           -- 到期日
    payment_pct     NUMERIC(5,2)   NOT NULL,           -- 付款百分比 30.00 = 30%
    payment_amount  NUMERIC(20,4)  NOT NULL,           -- 付款金额（含税总额 × 百分比）
    paid_amount     NUMERIC(20,4)  NOT NULL DEFAULT 0, -- 已付金额
    description     TEXT           NOT NULL DEFAULT '',-- 说明（如"预付款"、"尾款"）
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pps_order ON purchase_payment_schedules (order_id);

-- ============================================================================
-- 2. 采购参数配置表（单行配置，参考 ERPNext Buying Settings）
-- ============================================================================

CREATE TABLE purchase_settings (
    id                              BIGSERIAL   PRIMARY KEY,
    -- 收货容差
    over_delivery_allowance_pct     NUMERIC(5,2) NOT NULL DEFAULT 0,    -- 超收容差百分比
    over_shortage_allowance_pct     NUMERIC(5,2) NOT NULL DEFAULT 0,    -- 少收容差百分比
    -- 价格一致性
    maintain_same_rate              BOOLEAN     NOT NULL DEFAULT FALSE, -- 全链路保持相同单价
    -- 必填校验
    po_required_for_receipt         BOOLEAN     NOT NULL DEFAULT FALSE, -- 收货必须关联 PO
    receipt_required_for_invoice    BOOLEAN     NOT NULL DEFAULT FALSE, -- 付款必须关联对账/收货
    -- 默认值
    default_currency_code           VARCHAR(3)  NOT NULL DEFAULT 'CNY',
    default_tax_rate_id             BIGINT,
    -- 时间戳
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 初始化单行数据
INSERT INTO purchase_settings (id) VALUES (1);

-- ============================================================================
-- 3. PO 主表增加付款条款模板关联（可选）
-- ============================================================================

ALTER TABLE purchase_orders
    ADD COLUMN payment_schedule_generated BOOLEAN NOT NULL DEFAULT FALSE;
    -- 标记是否已根据条款模板生成付款计划

COMMIT;
```

---

## 三、付款计划 Service

### 3.1 新建 `abt-core/src/purchase/payment_schedule/` 模块

```
abt-core/src/purchase/payment_schedule/
├── mod.rs       // factory: new_payment_schedule_service(pool)
├── service.rs   // trait PaymentScheduleService
├── model.rs     // PaymentSchedule entity
└── repo.rs      // PaymentScheduleRepo: CRUD
```

### 3.2 Service Trait

```rust
#[async_trait]
pub trait PaymentScheduleService: Send + Sync {
    /// 根据 PO 含税总额和分期配置，生成付款计划行
    async fn generate_for_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        schedule_input: Vec<PaymentScheduleInput>,
    ) -> Result<Vec<i64>>;

    /// 查询某 PO 的付款计划
    async fn list_by_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<PaymentSchedule>>;

    /// 更新已付金额（付款确认时调用）
    async fn update_paid_amount(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        schedule_id: i64,
        paid_amount: Decimal,
    ) -> Result<()>;

    /// 删除（PO 取消或重新生成时调用）
    async fn delete_by_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<()>;
}

pub struct PaymentScheduleInput {
    pub due_date: NaiveDate,
    pub payment_pct: Decimal,    // 百分比
    pub description: String,
}
```

### 3.3 生成逻辑

在 PO `confirm()` 时调用 `generate_for_order`：

```rust
// order/implt.rs confirm() 中，状态转换成功后
if !req.payment_schedule.is_empty() {
    let total = order.amount_total;
    for input in &req.payment_schedule {
        let payment_amount = total * input.payment_pct / Decimal::from(100);
        // insert into purchase_payment_schedules
    }
}
```

**校验**：所有分期百分比之和必须 = 100%。

### 3.4 付款联动

在 `PaymentRequestServiceImpl::approve()` 中，将付款金额分配到 payment_schedule 行：

```rust
// payment/implt.rs approve() 中
let schedules = PaymentScheduleRepo::list_by_order(&mut *db, order_id).await?;

// 按到期日顺序分配付款金额
let mut remaining = req.amount;
for sched in schedules.iter().filter(|s| s.paid_amount < s.payment_amount) {
    let to_pay = remaining.min(sched.payment_amount - sched.paid_amount);
    PaymentScheduleRepo::update_paid(&mut *db, sched.id, sched.paid_amount + to_pay).await?;
    remaining -= to_pay;
    if remaining <= Decimal::ZERO { break; }
}
```

---

## 四、超收容差

### 4.1 PurchaseSettings Service

新建 `abt-core/src/purchase/settings/` 模块：

```rust
#[async_trait]
pub trait PurchaseSettingsService: Send + Sync {
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<PurchaseSettings>;
    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: UpdatePurchaseSettingsRequest) -> Result<()>;
}
```

### 4.2 arrival_handler 修改

**文件**：`abt-core/src/purchase/arrival_handler.rs`

在收货处理中增加容差校验：

```rust
// 3.5 校验超收容差
let settings = new_purchase_settings_service(self.pool.clone())
    .get(&ctx, &mut conn).await?;

for item in &po_items {
    let max_qty = item.quantity * (Decimal::ONE 
        + settings.over_delivery_allowance_pct / Decimal::from(100));
    
    if item.received_qty > max_qty {
        return Err(DomainError::validation(format!(
            "订单行 {} 收货数量 {} 超过允许上限 {}（含 {}% 容差）",
            item.line_no, item.received_qty, max_qty, 
            settings.over_delivery_allowance_pct
        )));
    }
}
```

### 4.3 价格一致性校验（maintain_same_rate）

在 PO confirm 时，如果关联了报价单，校验单价一致：

```rust
if settings.maintain_same_rate {
    for item in &items {
        if let Some(qi_id) = item.quotation_item_id {
            let quotation_item = PurchaseQuotationItemRepo::get_by_id(&mut *db, qi_id).await?;
            if item.unit_price != quotation_item.unit_price {
                return Err(DomainError::validation(format!(
                    "订单行 {} 单价 {} 与报价单单价 {} 不一致（已启用价格一致性校验）",
                    item.line_no, item.unit_price, quotation_item.unit_price
                )));
            }
        }
    }
}
```

---

## 五、Web 层变更

### 5.1 PO 创建/编辑页面

增加付款计划编辑区（PO 确认前可编辑）：
- 每行：到期日选择器 + 付款百分比输入 + 自动计算金额
- 底部显示百分比合计（必须 100%）

### 5.2 采购设置页面

新建 `purchase_settings` 页面：
- 超收容差百分比输入
- 超下单容差百分比输入
- 价格一致性校验开关
- 收货必关联 PO 开关

### 5.3 PO 详情页

在详情页增加付款计划展示区：
- 显示各期到期日、应付金额、已付金额、状态
- 可视化付款进度

---

## 六、验收标准

1. PO 确认时自动生成付款计划行，各期百分比之和 = 100%
2. 付款审批时，金额按到期日顺序分配到各期
3. 超收超过容差时，arrival_handler 返回错误
4. 开启 maintain_same_rate 后，PO 单价与报价单不一致时 confirm 失败
5. 采购设置页面可修改参数并立即生效
6. `cargo clippy` 通过

---

## 七、实施步骤

| 步骤 | 内容 | 工作量 |
|---|---|---|
| 1 | Migration `048_purchase_payment_terms_tolerance.sql` | 0.5h |
| 2 | 新建 `payment_schedule/` 模块（4 文件） | 2h |
| 3 | 新建 `settings/` 模块（4 文件） | 1.5h |
| 4 | 修改 `arrival_handler.rs` 增加容差校验 | 1h |
| 5 | 修改 `order/implt.rs` confirm 时生成付款计划 | 1h |
| 6 | 修改 `payment/implt.rs` approve 时联动付款计划 | 1h |
| 7 | 修改 `order/implt.rs` confirm 时价格一致性校验 | 0.5h |
| 8 | Web 页面：付款计划编辑 + 展示 | 2h |
| 9 | Web 页面：采购设置页 | 1h |
| 10 | 更新设计文档 | 1h |
| **合计** | | **11.5h** |
