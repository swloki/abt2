-- 月对账单主表
CREATE TABLE reconciliation_statements (
    statement_id     BIGSERIAL PRIMARY KEY,
    statement_no     VARCHAR(32) NOT NULL UNIQUE,
    customer_name    VARCHAR(200) NOT NULL,
    period_year      SMALLINT NOT NULL,
    period_month     SMALLINT NOT NULL,
    shipping_total   DECIMAL(14,2) NOT NULL DEFAULT 0,
    return_total     DECIMAL(14,2) NOT NULL DEFAULT 0,
    adjustment_total DECIMAL(14,2) NOT NULL DEFAULT 0,
    net_amount       DECIMAL(14,2) NOT NULL DEFAULT 0,
    status           SMALLINT NOT NULL DEFAULT 1,
    remark           TEXT,
    operator_id      BIGINT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at       TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_reconciliation_period ON reconciliation_statements(customer_name, period_year, period_month) WHERE deleted_at IS NULL;

-- 对账单明细
CREATE TABLE reconciliation_items (
    item_id       BIGSERIAL PRIMARY KEY,
    statement_id  BIGINT NOT NULL REFERENCES reconciliation_statements(statement_id),
    source_type   VARCHAR(20) NOT NULL,
    source_id     BIGINT,
    product_id    BIGINT,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    quantity      DECIMAL(14,6) NOT NULL,
    unit_price    DECIMAL(14,6) NOT NULL,
    amount        DECIMAL(14,2) NOT NULL,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_reconciliation_items_statement ON reconciliation_items(statement_id);
