-- 销售订单主表
CREATE TABLE sales_orders (
    order_id       BIGSERIAL PRIMARY KEY,
    order_no       VARCHAR(32) NOT NULL UNIQUE,
    quotation_id   BIGINT,
    customer_name  VARCHAR(200) NOT NULL,
    contact_person VARCHAR(100),
    contact_phone  VARCHAR(50),
    status         SMALLINT NOT NULL DEFAULT 1,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark         TEXT,
    delivery_date  TIMESTAMPTZ,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_sales_orders_status ON sales_orders(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_orders_customer ON sales_orders(customer_name) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_orders_quotation ON sales_orders(quotation_id) WHERE deleted_at IS NULL;

-- 销售订单行项目
CREATE TABLE sales_order_items (
    item_id       BIGSERIAL PRIMARY KEY,
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    product_id    BIGINT NOT NULL,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    unit_price    DECIMAL(14,6) NOT NULL,
    quantity      DECIMAL(14,6) NOT NULL,
    discount      DECIMAL(14,6) NOT NULL DEFAULT 0,
    subtotal      DECIMAL(14,2) NOT NULL,
    shipped_qty   DECIMAL(14,6) NOT NULL DEFAULT 0,
    returned_qty  DECIMAL(14,6) NOT NULL DEFAULT 0,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sales_order_items_order ON sales_order_items(order_id);
