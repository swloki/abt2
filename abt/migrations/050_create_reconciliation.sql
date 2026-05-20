BEGIN;

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

COMMENT ON TABLE reconciliation_statements IS '对账单主表';
COMMENT ON COLUMN reconciliation_statements.statement_no IS '系统生成编号，格式 RC-YYYY-MM-NNNNN';
COMMENT ON COLUMN reconciliation_statements.status IS '1=草稿,2=已确认,3=已取消';
COMMENT ON COLUMN reconciliation_statements.shipping_total IS '发货金额合计';
COMMENT ON COLUMN reconciliation_statements.return_total IS '退货金额合计';
COMMENT ON COLUMN reconciliation_statements.adjustment_total IS '调整金额';
COMMENT ON COLUMN reconciliation_statements.net_amount IS '净额 = shipping_total - return_total + adjustment_total';

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

COMMENT ON TABLE reconciliation_items IS '对账单行项目';
COMMENT ON COLUMN reconciliation_items.source_type IS '来源类型：shipping=发货, return=退货, adjustment=调整';
COMMENT ON COLUMN reconciliation_items.source_id IS '来源单据ID（发货申请ID或退货单ID）';
COMMENT ON COLUMN reconciliation_items.amount IS '金额 = unit_price * quantity';

COMMIT;
