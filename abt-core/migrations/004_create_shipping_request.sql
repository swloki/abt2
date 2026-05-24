-- shipping_requests 主表
CREATE TABLE shipping_requests (
    id                  BIGSERIAL PRIMARY KEY,
    doc_number          VARCHAR(30) NOT NULL UNIQUE,
    order_id            BIGINT NOT NULL,
    customer_id         BIGINT NOT NULL,
    request_date        DATE NOT NULL DEFAULT CURRENT_DATE,
    expected_ship_date  DATE,
    status              SMALLINT NOT NULL DEFAULT 1,
    shipping_address    TEXT NOT NULL DEFAULT '',
    carrier             VARCHAR(100) NOT NULL DEFAULT '',
    tracking_number     VARCHAR(100) NOT NULL DEFAULT '',
    remark              TEXT NOT NULL DEFAULT '',
    operator_id         BIGINT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ
);

CREATE INDEX idx_shipping_requests_order ON shipping_requests(order_id);
CREATE INDEX idx_shipping_requests_customer ON shipping_requests(customer_id);
CREATE INDEX idx_shipping_requests_status ON shipping_requests(status);
CREATE INDEX idx_shipping_requests_doc_number ON shipping_requests(doc_number);

-- shipping_request_items 明细表
CREATE TABLE shipping_request_items (
    id                      BIGSERIAL PRIMARY KEY,
    shipping_request_id     BIGINT NOT NULL REFERENCES shipping_requests(id),
    line_no                 INT NOT NULL,
    order_item_id           BIGINT NOT NULL,
    product_id              BIGINT NOT NULL,
    warehouse_id            BIGINT NOT NULL,
    requested_qty           DECIMAL(18,6) NOT NULL,
    shipped_qty             DECIMAL(18,6) NOT NULL DEFAULT 0,
    description             TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_shipping_request_items_sr ON shipping_request_items(shipping_request_id);
