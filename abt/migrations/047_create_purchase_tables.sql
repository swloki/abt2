CREATE TABLE purchase_orders (
    po_id          BIGSERIAL PRIMARY KEY,
    po_no          VARCHAR(32) NOT NULL UNIQUE,
    supplier_id    BIGINT NOT NULL,
    order_type     SMALLINT NOT NULL DEFAULT 1,
    status         SMALLINT NOT NULL DEFAULT 1,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_purchase_orders_supplier ON purchase_orders(supplier_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_purchase_orders_status ON purchase_orders(status) WHERE deleted_at IS NULL;

CREATE TABLE purchase_order_items (
    item_id        BIGSERIAL PRIMARY KEY,
    po_id          BIGINT NOT NULL REFERENCES purchase_orders(po_id) ON DELETE CASCADE,
    product_id     BIGINT NOT NULL,
    product_code   VARCHAR(100),
    product_name   VARCHAR(200),
    unit           VARCHAR(20),
    unit_price     DECIMAL(14,6) NOT NULL,
    quantity       DECIMAL(14,6) NOT NULL,
    received_qty   DECIMAL(14,6) NOT NULL DEFAULT 0,
    subtotal       DECIMAL(14,2) NOT NULL,
    remark         TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_purchase_order_items_po ON purchase_order_items(po_id);

CREATE TABLE purchase_statements (
    statement_id   BIGSERIAL PRIMARY KEY,
    statement_no   VARCHAR(32) NOT NULL UNIQUE,
    supplier_id    BIGINT NOT NULL,
    period_start   DATE NOT NULL,
    period_end     DATE NOT NULL,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE purchase_statement_items (
    item_id          BIGSERIAL PRIMARY KEY,
    statement_id     BIGINT NOT NULL REFERENCES purchase_statements(statement_id) ON DELETE CASCADE,
    po_id            BIGINT NOT NULL,
    po_no            VARCHAR(32),
    product_id       BIGINT NOT NULL,
    product_name     VARCHAR(200),
    quantity         DECIMAL(14,6) NOT NULL,
    unit_price       DECIMAL(14,6) NOT NULL,
    amount           DECIMAL(14,2) NOT NULL
);

CREATE INDEX idx_statement_items_statement ON purchase_statement_items(statement_id);

CREATE TABLE purchase_invoices (
    invoice_id     BIGSERIAL PRIMARY KEY,
    invoice_no     VARCHAR(100) NOT NULL,
    supplier_id    BIGINT NOT NULL,
    statement_id   BIGINT,
    invoice_amount DECIMAL(14,2) NOT NULL,
    invoice_date   DATE NOT NULL,
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE purchase_payments (
    payment_id     BIGSERIAL PRIMARY KEY,
    payment_no     VARCHAR(32) NOT NULL UNIQUE,
    supplier_id    BIGINT NOT NULL,
    invoice_id     BIGINT,
    payment_amount DECIMAL(14,2) NOT NULL,
    payment_method VARCHAR(50),
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
