-- sales_returns 主表
CREATE TABLE sales_returns (
    id                      BIGSERIAL PRIMARY KEY,
    doc_number              VARCHAR(30) NOT NULL UNIQUE,
    order_id                BIGINT NOT NULL,
    shipping_request_id     BIGINT NOT NULL,
    customer_id             BIGINT NOT NULL,
    return_date             DATE NOT NULL DEFAULT CURRENT_DATE,
    status                  SMALLINT NOT NULL DEFAULT 1,
    return_reason           TEXT NOT NULL DEFAULT '',
    total_amount             DECIMAL(20,4) NOT NULL DEFAULT 0,
    remark                  TEXT NOT NULL DEFAULT '',
    operator_id             BIGINT NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at              TIMESTAMPTZ
);

CREATE INDEX idx_sales_returns_order ON sales_returns(order_id);
CREATE INDEX idx_sales_returns_shipping ON sales_returns(shipping_request_id);
CREATE INDEX idx_sales_returns_customer ON sales_returns(customer_id);
CREATE INDEX idx_sales_returns_status ON sales_returns(status);
CREATE INDEX idx_sales_returns_doc_number ON sales_returns(doc_number);

-- sales_return_items 明细表
CREATE TABLE sales_return_items (
    id              BIGSERIAL PRIMARY KEY,
    return_id       BIGINT NOT NULL REFERENCES sales_returns(id),
    order_item_id   BIGINT NOT NULL,
    product_id      BIGINT NOT NULL,
    returned_qty    DECIMAL(18,6) NOT NULL,
    unit_price      DECIMAL(18,6) NOT NULL,
    amount          DECIMAL(20,4) NOT NULL,
    disposition     SMALLINT NOT NULL DEFAULT 1
);

CREATE INDEX idx_sales_return_items_return ON sales_return_items(return_id);
