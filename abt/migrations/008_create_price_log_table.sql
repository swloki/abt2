-- abt/migrations/008_create_price_log_table.sql
-- 价格日志表迁移
-- 注意：现有代码库使用 products 表（复数形式）

CREATE TABLE IF NOT EXISTS product_price_log (
    log_id BIGSERIAL PRIMARY KEY,
    product_id BIGINT NOT NULL REFERENCES products(product_id) ON DELETE CASCADE,
    old_price DECIMAL(18,4),
    new_price DECIMAL(18,4) NOT NULL,
    operator_id BIGINT,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_price_log_product ON product_price_log(product_id);
CREATE INDEX IF NOT EXISTS idx_price_log_created ON product_price_log(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_price_log_operator ON product_price_log(operator_id);

COMMENT ON TABLE product_price_log IS '产品价格变动日志表';
COMMENT ON COLUMN product_price_log.log_id IS '日志ID';
COMMENT ON COLUMN product_price_log.product_id IS '产品ID';
COMMENT ON COLUMN product_price_log.old_price IS '变动前价格';
COMMENT ON COLUMN product_price_log.new_price IS '变动后价格';
COMMENT ON COLUMN product_price_log.operator_id IS '操作人用户ID';
COMMENT ON COLUMN product_price_log.remark IS '备注';
