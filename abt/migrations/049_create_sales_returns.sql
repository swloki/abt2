BEGIN;

CREATE TABLE sales_returns (
    return_id     BIGSERIAL PRIMARY KEY,
    return_no     VARCHAR(32) NOT NULL UNIQUE,
    request_id    BIGINT NOT NULL REFERENCES shipping_requests(request_id),
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    customer_name VARCHAR(200) NOT NULL,
    status        SMALLINT NOT NULL DEFAULT 1,
    total_amount  DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark        TEXT,
    reason        TEXT,
    operator_id   BIGINT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);

CREATE INDEX idx_sales_returns_request ON sales_returns(request_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_returns_order ON sales_returns(order_id) WHERE deleted_at IS NULL;

COMMENT ON TABLE sales_returns IS '销售退货主表';
COMMENT ON COLUMN sales_returns.return_no IS '系统生成编号，格式 RT-YYYY-MM-NNNNN';
COMMENT ON COLUMN sales_returns.status IS '1=草稿,2=已确认,3=已入库,4=已取消';
COMMENT ON COLUMN sales_returns.reason IS '退货原因';

CREATE TABLE sales_return_items (
    item_id         BIGSERIAL PRIMARY KEY,
    return_id       BIGINT NOT NULL REFERENCES sales_returns(return_id),
    request_item_id BIGINT NOT NULL REFERENCES shipping_request_items(item_id),
    order_item_id   BIGINT NOT NULL REFERENCES sales_order_items(item_id),
    product_id      BIGINT NOT NULL,
    product_code    VARCHAR(100),
    product_name    VARCHAR(200),
    unit            VARCHAR(20),
    unit_price      DECIMAL(14,6) NOT NULL,
    quantity        DECIMAL(14,6) NOT NULL,
    subtotal        DECIMAL(14,2) NOT NULL,
    remark          TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sales_return_items_return ON sales_return_items(return_id);

COMMENT ON TABLE sales_return_items IS '销售退货行项目';
COMMENT ON COLUMN sales_return_items.request_item_id IS '关联发货申请行项目ID';
COMMENT ON COLUMN sales_return_items.order_item_id IS '关联销售订单行项目ID';
COMMENT ON COLUMN sales_return_items.subtotal IS '小计 = unit_price * quantity';

COMMIT;
