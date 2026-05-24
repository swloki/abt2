-- sales_orders 主表
CREATE TABLE sales_orders (
    id              BIGSERIAL PRIMARY KEY,
    doc_number      VARCHAR(30) NOT NULL UNIQUE,
    customer_id     BIGINT NOT NULL,
    contact_id      BIGINT NOT NULL,
    sales_rep_id    BIGINT NOT NULL,
    order_date      DATE NOT NULL DEFAULT CURRENT_DATE,
    status          SMALLINT NOT NULL DEFAULT 1,
    total_amount    DECIMAL(20,4) NOT NULL DEFAULT 0,
    total_cost      DECIMAL(20,4) NOT NULL DEFAULT 0,
    payment_terms   VARCHAR(100) NOT NULL DEFAULT '',
    delivery_terms  VARCHAR(100) NOT NULL DEFAULT '',
    delivery_address TEXT NOT NULL DEFAULT '',
    remark          TEXT NOT NULL DEFAULT '',
    operator_id     BIGINT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_sales_orders_customer ON sales_orders(customer_id);
CREATE INDEX idx_sales_orders_status ON sales_orders(status);
CREATE INDEX idx_sales_orders_doc_number ON sales_orders(doc_number);

-- sales_order_items 明细表
CREATE TABLE sales_order_items (
    id              BIGSERIAL PRIMARY KEY,
    order_id        BIGINT NOT NULL REFERENCES sales_orders(id),
    line_no         INT NOT NULL,
    product_id      BIGINT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    quantity        DECIMAL(18,6) NOT NULL,
    unit            VARCHAR(20) NOT NULL DEFAULT '',
    unit_price      DECIMAL(18,6) NOT NULL,
    unit_cost       DECIMAL(18,6) NOT NULL DEFAULT 0,
    discount_rate   DECIMAL(5,2) NOT NULL DEFAULT 0,
    amount          DECIMAL(20,4) NOT NULL,
    shipped_qty     DECIMAL(18,6) NOT NULL DEFAULT 0,
    returned_qty    DECIMAL(18,6) NOT NULL DEFAULT 0,
    delivery_date   DATE
);

CREATE INDEX idx_sales_order_items_order ON sales_order_items(order_id);
