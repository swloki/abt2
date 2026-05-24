-- reconciliations 主表
CREATE TABLE reconciliations (
    id                  BIGSERIAL PRIMARY KEY,
    doc_number          VARCHAR(30) NOT NULL UNIQUE,
    customer_id         BIGINT NOT NULL,
    period              VARCHAR(7) NOT NULL,
    status              SMALLINT NOT NULL DEFAULT 1,
    total_amount        DECIMAL(20,4) NOT NULL DEFAULT 0,
    confirmed_amount    DECIMAL(20,4) NOT NULL DEFAULT 0,
    difference          DECIMAL(20,4) NOT NULL DEFAULT 0,
    remark              TEXT NOT NULL DEFAULT '',
    operator_id         BIGINT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ,
    UNIQUE (customer_id, period)
);

CREATE INDEX idx_reconciliations_customer ON reconciliations(customer_id);
CREATE INDEX idx_reconciliations_period ON reconciliations(period);
CREATE INDEX idx_reconciliations_status ON reconciliations(status);
CREATE INDEX idx_reconciliations_doc_number ON reconciliations(doc_number);

-- reconciliation_items 明细表
CREATE TABLE reconciliation_items (
    id                      BIGSERIAL PRIMARY KEY,
    reconciliation_id       BIGINT NOT NULL REFERENCES reconciliations(id),
    shipping_request_id     BIGINT NOT NULL,
    sales_order_id          BIGINT NOT NULL,
    product_id              BIGINT NOT NULL,
    quantity                DECIMAL(18,6) NOT NULL,
    unit_price              DECIMAL(18,6) NOT NULL,
    amount                  DECIMAL(20,4) NOT NULL,
    confirmed               BOOLEAN NOT NULL DEFAULT FALSE,
    remark                  TEXT
);

CREATE INDEX idx_reconciliation_items_rec ON reconciliation_items(reconciliation_id);
