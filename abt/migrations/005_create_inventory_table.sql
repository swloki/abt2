-- 库存表
CREATE TABLE IF NOT EXISTS inventory (
    inventory_id BIGSERIAL PRIMARY KEY,
    product_id BIGINT NOT NULL,
    location_id BIGINT NOT NULL,
    quantity BIGINT NOT NULL DEFAULT 0,
    safety_stock BIGINT NOT NULL DEFAULT 0,
    batch_no VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,

    UNIQUE(product_id, location_id)
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_inventory_product ON inventory(product_id);
CREATE INDEX IF NOT EXISTS idx_inventory_location ON inventory(location_id);
CREATE INDEX IF NOT EXISTS idx_inventory_low_stock ON inventory(quantity, safety_stock);

-- 注释
COMMENT ON TABLE inventory IS '库存表';
COMMENT ON COLUMN inventory.inventory_id IS '库存ID';
COMMENT ON COLUMN inventory.product_id IS '产品ID (关联 product.product_id)';
COMMENT ON COLUMN inventory.location_id IS '库位ID (关联 location.location_id)';
COMMENT ON COLUMN inventory.quantity IS '库存数量';
COMMENT ON COLUMN inventory.safety_stock IS '安全库存（预警阈值）';
COMMENT ON COLUMN inventory.batch_no IS '批次号（可选）';
