BEGIN;

-- ============================================================================
-- 1. 新建税率表
-- ============================================================================

CREATE TABLE tax_rates (
    id              BIGSERIAL      PRIMARY KEY,
    code            VARCHAR(16)    NOT NULL,
    name            VARCHAR(64)    NOT NULL,
    rate            NUMERIC(5,2)   NOT NULL,
    tax_type        SMALLINT       NOT NULL DEFAULT 1, -- 1=Purchase, 2=Sales, 3=Both
    is_active       BOOLEAN        NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_tax_rates_code ON tax_rates (code) WHERE deleted_at IS NULL;

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
