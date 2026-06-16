# P1a: 采购模块税务 + 折扣 + 多币种实现计划

> 日期：2026-06-15  
> 优先级：P1（核心功能补齐）  
> 参考：Odoo `purchase_order.py` / `purchase_order_line.py`；ERPNext `purchase_order.json` / `purchase_taxes_and_charges`  
> 改动范围：migration + model + repo + implt + 页面

---

## 一、设计参考

### Odoo 实现

- **PO 行**：`discount Float`（百分比折扣），`tax_ids Many2many(account.tax)`（多税码），`price_subtotal`（税前小计），`price_total`（含税总计），`price_tax`（税额）
- **PO 主表**：`amount_untaxed`（不含税金额），`amount_tax`（税额合计），`amount_total`（含税总计），`currency_id`，`currency_rate`（存储汇率快照）
- **金额计算**：`_compute_amount()` 逐行计算 `price_subtotal = qty * price_unit * (1 - discount/100)`，然后通过 `account.tax` 计算税额

### ERPNext 实现

- **PO 行**：`discount_percentage Float`，`rate`（单价），`qty`，`amount`（= rate * qty），`base_rate`（本币单价）
- **PO 主表**：`currency`，`conversion_rate`（汇率），`net_total`（不含税），`grand_total`（含税），`taxes` 子表（`PurchaseTaxesandCharges`）
- **税务模板**：`taxes_and_charges` 字段引用 `Purchase Taxes and Charges Template`，自动填充 taxes 子表

### ABT 设计决策

采用**简化版 Odoo 模型**：
- PO 行增加 `discount_pct`（百分比折扣）和 `tax_rate_id`（关联税率表）
- PO 主表增加 `currency_code`、`currency_rate`、`amount_untaxed`、`amount_tax`、`amount_total`
- 新建 `tax_rates` 独立表（税率码 + 税率 + 类型）
- 折扣仅支持百分比（Odoo 模式），不支持 ERPNext 的"折扣作用于总额/净额"配置（YAGNI）

---

## 二、数据模型变更

### Migration: `047_purchase_tax_discount_currency.sql`

```sql
BEGIN;

-- ============================================================================
-- 1. 新建税率表（简化版，不含 fiscal_position 自动映射）
-- ============================================================================

CREATE TABLE tax_rates (
    id              BIGSERIAL      PRIMARY KEY,
    code            VARCHAR(16)    NOT NULL,        -- 如 'VAT13', 'VAT9', 'VAT0'
    name            VARCHAR(64)    NOT NULL,        -- 显示名 '增值税 13%'
    rate            NUMERIC(5,2)   NOT NULL,        -- 税率百分比 13.00
    tax_type        SMALLINT       NOT NULL DEFAULT 1, -- 1=Purchase(进项), 2=Sales(销项), 3=Both
    is_active       BOOLEAN        NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_tax_rates_code ON tax_rates (code) WHERE deleted_at IS NULL;

-- 初始税率数据（中国大陆常用）
INSERT INTO tax_rates (code, name, rate, tax_type) VALUES
    ('VAT13', '增值税 13%', 13.00, 3),
    ('VAT9',  '增值税 9%',   9.00, 3),
    ('VAT6',  '增值税 6%',   6.00, 3),
    ('VAT3',  '增值税 3%（小规模）', 3.00, 3),
    ('VAT0',  '免税 0%',     0.00, 3);

-- ============================================================================
-- 2. PO 主表增加多币种 + 金额字段
-- ============================================================================

ALTER TABLE purchase_orders
    ADD COLUMN currency_code    VARCHAR(3)     NOT NULL DEFAULT 'CNY',
    ADD COLUMN currency_rate    NUMERIC(18,8)  NOT NULL DEFAULT 1.0,
    ADD COLUMN amount_untaxed   NUMERIC(20,4)  NOT NULL DEFAULT 0,
    ADD COLUMN amount_tax       NUMERIC(20,4)  NOT NULL DEFAULT 0,
    ADD COLUMN amount_total     NUMERIC(20,4)  NOT NULL DEFAULT 0,
    ADD COLUMN discount_amount  NUMERIC(20,4)  NOT NULL DEFAULT 0;

-- 将已有数据的 amount_untaxed/amount_total 设为原 total_amount
UPDATE purchase_orders SET
    amount_untaxed = total_amount,
    amount_total = total_amount;

-- ============================================================================
-- 3. PO 明细增加折扣 + 税率关联
-- ============================================================================

ALTER TABLE purchase_order_items
    ADD COLUMN discount_pct   NUMERIC(5,2)  NOT NULL DEFAULT 0,
    ADD COLUMN tax_rate_id    BIGINT,
    ADD COLUMN price_subtotal NUMERIC(20,4) NOT NULL DEFAULT 0,
    ADD COLUMN price_tax      NUMERIC(20,4) NOT NULL DEFAULT 0,
    ADD COLUMN price_total    NUMERIC(20,4) NOT NULL DEFAULT 0;

-- 回填已有明细的 price_subtotal = amount, price_total = amount
UPDATE purchase_order_items SET
    price_subtotal = amount,
    price_total = amount;

CREATE INDEX idx_poi_tax_rate ON purchase_order_items (tax_rate_id) WHERE tax_rate_id IS NOT NULL;

-- ============================================================================
-- 4. 报价单明细也增加税率（保持一致性）
-- ============================================================================

ALTER TABLE purchase_quotation_items
    ADD COLUMN discount_pct   NUMERIC(5,2)  NOT NULL DEFAULT 0,
    ADD COLUMN tax_rate_id    BIGINT;

COMMIT;
```

---

## 三、Rust Model 变更

### 3.1 新建 `abt-core/src/shared/enums/tax.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[repr(i16)]
pub enum TaxType {
    Purchase = 1,
    Sales = 2,
    Both = 3,
}
```

### 3.2 新建 `abt-core/src/purchase/tax/` 模块

```
abt-core/src/purchase/tax/
├── mod.rs       // factory: new_tax_rate_service(pool) -> impl TaxRateService
├── service.rs   // trait TaxRateService: list_active(), get_by_code()
├── model.rs     // TaxRate entity
└── repo.rs      // TaxRateRepo: CRUD
```

### 3.3 PO Model 变更

**`abt-core/src/purchase/order/model.rs`**

PurchaseOrder 实体新增字段：
```rust
pub struct PurchaseOrder {
    // ... 已有字段 ...
    pub currency_code: String,       // "CNY", "USD"
    pub currency_rate: Decimal,      // 汇率快照（下单时锁定）
    pub amount_untaxed: Decimal,     // 不含税金额
    pub amount_tax: Decimal,         // 税额合计
    pub amount_total: Decimal,       // 含税总计
    pub discount_amount: Decimal,    // 整单折扣金额
}
```

PurchaseOrderItem 实体新增字段：
```rust
pub struct PurchaseOrderItem {
    // ... 已有字段 ...
    pub discount_pct: Decimal,       // 行级折扣百分比 0.00 ~ 100.00
    pub tax_rate_id: Option<i64>,    // 关联 tax_rates.id
    pub price_subtotal: Decimal,     // 税前小计 = qty * price * (1 - discount/100)
    pub price_tax: Decimal,          // 行税额 = subtotal * tax_rate
    pub price_total: Decimal,        // 含税总计 = subtotal + tax
}
```

CreateOrderItemRequest 新增字段：
```rust
pub struct CreateOrderItemRequest {
    // ... 已有字段 ...
    pub discount_pct: Decimal,       // 默认 0
    pub tax_rate_id: Option<i64>,    // 默认 None
}
```

CreatePurchaseOrderRequest 新增字段：
```rust
pub struct CreatePurchaseOrderRequest {
    // ... 已有字段 ...
    pub currency_code: String,       // 默认 "CNY"
    pub currency_rate: Decimal,      // 默认 Decimal::ONE
    pub discount_amount: Decimal,    // 默认 Decimal::ZERO
}
```

### 3.4 金额计算逻辑

**`abt-core/src/purchase/order/implt.rs`** 的 `create()` / `update()` 中：

```rust
// 逐行计算
for item in &req.items {
    let price_subtotal = item.quantity * item.unit_price
        * (Decimal::ONE - item.discount_pct / Decimal::from(100));
    
    let (price_tax, price_total) = if let Some(tax_rate_id) = item.tax_rate_id {
        let tax_rate = TaxRateRepo::get_by_id(&mut *db, tax_rate_id).await?
            .ok_or_else(|| DomainError::validation("税率不存在"))?;
        let tax = price_subtotal * tax_rate.rate / Decimal::from(100);
        (tax, price_subtotal + tax)
    } else {
        (Decimal::ZERO, price_subtotal)
    };
    
    // 汇总
    amount_untaxed += price_subtotal;
    amount_tax += price_tax;
}

// 整单折扣
amount_untaxed -= req.discount_amount;
amount_total = amount_untaxed + amount_tax;
```

---

## 四、Repo 变更

### 4.1 `order/repo.rs` — insert/update SQL 增加新字段

**PurchaseOrderRepo::insert** 增加 6 个字段：
```sql
INSERT INTO purchase_orders
    (doc_number, supplier_id, order_date, expected_delivery_date, status,
     total_amount, payment_terms, delivery_address, remark, operator_id,
     currency_code, currency_rate, amount_untaxed, amount_tax, amount_total, discount_amount)
VALUES ($1, ..., $16)
```

**PurchaseOrderItemRepo::insert_items** 增加 4 个字段：
```sql
INSERT INTO purchase_order_items
    (order_id, line_no, product_id, description, quantity, unit_price, amount,
     quotation_item_id, expected_delivery_date,
     discount_pct, tax_rate_id, price_subtotal, price_tax, price_total)
VALUES ($1, ..., $14)
```

**list_by_order_id / list_received_by_supplier** 的 SELECT 也增加新字段。

---

## 五、Web 层变更

### 5.1 页面

- `purchase_order_create.rs` / `purchase_order_edit.rs`：增加币种下拉、汇率输入、整单折扣输入
- 行项目编辑区：每行增加折扣%输入和税率下拉
- 金额汇总区：显示 不含税金额 / 税额 / 含税总计

### 5.2 前端计算（Hyperscript + JS）

行金额实时计算（纯前端，不调服务器）：
```javascript
// static/app.js 增加函数
function calcPurchaseLine(row) {
    const qty = parseFloat(row.qty.value) || 0;
    const price = parseFloat(row.price.value) || 0;
    const discount = parseFloat(row.discount.value) || 0;
    const taxRate = parseFloat(row.taxRate.value) || 0;
    
    const subtotal = qty * price * (1 - discount / 100);
    const tax = subtotal * taxRate / 100;
    const total = subtotal + tax;
    
    row.subtotal.textContent = formatMoney(subtotal);
    row.tax.textContent = formatMoney(tax);
    row.total.textContent = formatMoney(total);
}
```

---

## 六、验收标准

1. 创建 PO 时可选择币种（默认 CNY），输入汇率后金额按汇率换算显示
2. 每行可输入折扣百分比，实时计算税前小计
3. 每行可选择税率（从 tax_rates 表加载），实时计算税额
4. 底部汇总区正确显示：不含税金额 = sum(price_subtotal) - discount_amount；税额 = sum(price_tax)；含税总计 = 不含税 + 税额
5. `cargo clippy` 通过
6. 数据库 migration 执行成功，已有数据正确回填

---

## 七、实施步骤

| 步骤 | 内容 | 工作量 |
|---|---|---|
| 1 | 编写 migration `047_purchase_tax_discount_currency.sql` | 0.5h |
| 2 | 新建 `purchase/tax/` 模块（service + model + repo） | 1h |
| 3 | 修改 `order/model.rs` 增加字段 | 0.5h |
| 4 | 修改 `order/repo.rs` SQL 增加字段 | 1h |
| 5 | 修改 `order/implt.rs` 增加金额计算逻辑 | 1h |
| 6 | 修改 `order/service.rs` trait 签名（如有） | 0.5h |
| 7 | 修改 Web 页面 create/edit 增加新字段 | 1.5h |
| 8 | 前端 JS 计算逻辑 | 1h |
| 9 | 更新设计文档 `docs/uml-design/02-purchase.html` | 1h |
| **合计** | | **8h** |
