BEGIN;

-- ============================================================================
-- 1. 付款计划子表（PO 关联子表）
-- ============================================================================

CREATE TABLE purchase_payment_schedules (
    id              BIGSERIAL      PRIMARY KEY,
    order_id        BIGINT         NOT NULL,
    line_no         INTEGER        NOT NULL,
    due_date        DATE           NOT NULL,
    payment_pct     NUMERIC(5,2)   NOT NULL,
    payment_amount  NUMERIC(20,4)  NOT NULL,
    paid_amount     NUMERIC(20,4)  NOT NULL DEFAULT 0,
    description     TEXT           NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pps_order ON purchase_payment_schedules (order_id);

-- ============================================================================
-- 2. 采购参数配置表（单行配置）
-- ============================================================================

CREATE TABLE purchase_settings (
    id                              BIGSERIAL   PRIMARY KEY,
    over_delivery_allowance_pct     NUMERIC(5,2) NOT NULL DEFAULT 0,
    over_shortage_allowance_pct     NUMERIC(5,2) NOT NULL DEFAULT 0,
    maintain_same_rate              BOOLEAN     NOT NULL DEFAULT FALSE,
    po_required_for_receipt         BOOLEAN     NOT NULL DEFAULT FALSE,
    receipt_required_for_invoice    BOOLEAN     NOT NULL DEFAULT FALSE,
    default_currency_code           VARCHAR(3)  NOT NULL DEFAULT 'CNY',
    default_tax_rate_id             BIGINT,
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO purchase_settings (id) VALUES (1);

-- ============================================================================
-- 3. PO 主表增加付款计划标记
-- ============================================================================

ALTER TABLE purchase_orders
    ADD COLUMN payment_schedule_generated BOOLEAN NOT NULL DEFAULT FALSE;

COMMIT;
