CREATE TABLE IF NOT EXISTS product_watchers (
    user_id BIGINT NOT NULL,
    product_id BIGINT NOT NULL,
    safety_stock_override DECIMAL(18, 6) NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_product_watchers_product
    ON product_watchers(product_id);

COMMENT ON TABLE product_watchers IS '用户关注的产品列表';
COMMENT ON COLUMN product_watchers.safety_stock_override IS '用户自定义告警阈值，NULL 则使用 inventory.safety_stock';
