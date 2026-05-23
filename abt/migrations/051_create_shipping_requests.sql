-- 发货申请主表
CREATE TABLE shipping_requests (
    request_id    BIGSERIAL PRIMARY KEY,
    request_no    VARCHAR(32) NOT NULL UNIQUE,
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    customer_name VARCHAR(200) NOT NULL,
    status        SMALLINT NOT NULL DEFAULT 1,
    remark        TEXT,
    operator_id   BIGINT,
    confirmed_at  TIMESTAMPTZ,
    shipped_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);

CREATE INDEX idx_shipping_requests_order ON shipping_requests(order_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_shipping_requests_status ON shipping_requests(status) WHERE deleted_at IS NULL;

-- 发货申请行项目
CREATE TABLE shipping_request_items (
    item_id       BIGSERIAL PRIMARY KEY,
    request_id    BIGINT NOT NULL REFERENCES shipping_requests(request_id),
    order_item_id BIGINT NOT NULL REFERENCES sales_order_items(item_id),
    product_id    BIGINT NOT NULL,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    quantity      DECIMAL(14,6) NOT NULL,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_shipping_request_items_request ON shipping_request_items(request_id);
