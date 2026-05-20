BEGIN;

CREATE TABLE quotations (
    quotation_id   BIGSERIAL PRIMARY KEY,
    quotation_no   VARCHAR(32) NOT NULL UNIQUE,
    customer_name  VARCHAR(200) NOT NULL,
    contact_person VARCHAR(100),
    contact_phone  VARCHAR(50),
    status         SMALLINT NOT NULL DEFAULT 1,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark         TEXT,
    valid_until    TIMESTAMPTZ,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_quotations_status ON quotations(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_quotations_customer ON quotations(customer_name) WHERE deleted_at IS NULL;

COMMENT ON TABLE quotations IS '销售报价单主表';
COMMENT ON COLUMN quotations.quotation_no IS '系统生成编号，格式 QT-YYYY-MM-NNNNN';
COMMENT ON COLUMN quotations.status IS '1=草稿,2=已提交,3=已接受,4=已拒绝,5=已过期';

CREATE TABLE quotation_items (
    item_id       BIGSERIAL PRIMARY KEY,
    quotation_id  BIGINT NOT NULL REFERENCES quotations(quotation_id),
    product_id    BIGINT NOT NULL,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    unit_price    DECIMAL(14,6) NOT NULL,
    quantity      DECIMAL(14,6) NOT NULL,
    discount      DECIMAL(5,4) NOT NULL DEFAULT 1.0,
    subtotal      DECIMAL(14,2) NOT NULL,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_quotation_items_quotation ON quotation_items(quotation_id);

COMMENT ON TABLE quotation_items IS '报价单行项目';
COMMENT ON COLUMN quotation_items.discount IS '折扣率 0.0000~1.0000，1.0=无折扣';
COMMENT ON COLUMN quotation_items.subtotal IS '小计 = unit_price * quantity * discount';

COMMIT;
