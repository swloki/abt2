-- quotations 主表
CREATE TABLE quotations (
    id              BIGSERIAL PRIMARY KEY,
    doc_number      VARCHAR(30) NOT NULL UNIQUE,
    customer_id     BIGINT NOT NULL,
    contact_id      BIGINT NOT NULL,
    sales_rep_id    BIGINT NOT NULL,
    quotation_date  DATE NOT NULL DEFAULT CURRENT_DATE,
    valid_until     DATE NOT NULL,
    status          SMALLINT NOT NULL DEFAULT 1,
    total_amount    DECIMAL(20,4) NOT NULL DEFAULT 0,
    total_cost      DECIMAL(20,4) NOT NULL DEFAULT 0,
    estimated_margin DECIMAL(5,2) NOT NULL DEFAULT 0,
    payment_terms   VARCHAR(100) NOT NULL DEFAULT '',
    delivery_terms  VARCHAR(100) NOT NULL DEFAULT '',
    remark          TEXT NOT NULL DEFAULT '',
    operator_id     BIGINT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_quotations_customer ON quotations(customer_id);
CREATE INDEX idx_quotations_status ON quotations(status);
CREATE INDEX idx_quotations_doc_number ON quotations(doc_number);

-- quotation_items 明细表
CREATE TABLE quotation_items (
    id              BIGSERIAL PRIMARY KEY,
    quotation_id    BIGINT NOT NULL REFERENCES quotations(id),
    line_no         INT NOT NULL,
    product_id      BIGINT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    quantity        DECIMAL(18,6) NOT NULL,
    unit            VARCHAR(20) NOT NULL DEFAULT '',
    unit_price      DECIMAL(18,6) NOT NULL,
    unit_cost       DECIMAL(18,6) NOT NULL DEFAULT 0,
    discount_rate   DECIMAL(5,2) NOT NULL DEFAULT 0,
    amount          DECIMAL(20,4) NOT NULL,
    delivery_date   DATE
);

CREATE INDEX idx_quotation_items_quotation ON quotation_items(quotation_id);
