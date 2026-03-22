-- 库存变动日志表
CREATE TABLE IF NOT EXISTS inventory_log (
    log_id BIGSERIAL PRIMARY KEY,
    inventory_id BIGINT NOT NULL,
    product_id BIGINT NOT NULL,
    location_id BIGINT NOT NULL,
    change_qty BIGINT NOT NULL,
    before_qty BIGINT NOT NULL,
    after_qty BIGINT NOT NULL,
    operation_type VARCHAR(20) NOT NULL,
    ref_order_type VARCHAR(50),
    ref_order_id VARCHAR(100),
    operator VARCHAR(100),
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_inventory_log_inventory ON inventory_log(inventory_id);
CREATE INDEX IF NOT EXISTS idx_inventory_log_product ON inventory_log(product_id);
CREATE INDEX IF NOT EXISTS idx_inventory_log_created ON inventory_log(created_at);
CREATE INDEX IF NOT EXISTS idx_inventory_log_operation ON inventory_log(operation_type, created_at);

-- 注释
COMMENT ON TABLE inventory_log IS '库存变动日志表';
COMMENT ON COLUMN inventory_log.log_id IS '日志ID';
COMMENT ON COLUMN inventory_log.inventory_id IS '库存ID';
COMMENT ON COLUMN inventory_log.product_id IS '产品ID（冗余，便于查询）';
COMMENT ON COLUMN inventory_log.location_id IS '库位ID（冗余，便于查询）';
COMMENT ON COLUMN inventory_log.change_qty IS '变动数量（正数入库，负数出库）';
COMMENT ON COLUMN inventory_log.before_qty IS '变动前数量';
COMMENT ON COLUMN inventory_log.after_qty IS '变动后数量';
COMMENT ON COLUMN inventory_log.operation_type IS '操作类型: in/out/transfer/adjust';
COMMENT ON COLUMN inventory_log.ref_order_type IS '关联单据类型';
COMMENT ON COLUMN inventory_log.ref_order_id IS '关联单据ID';
COMMENT ON COLUMN inventory_log.operator IS '操作人';
COMMENT ON COLUMN inventory_log.remark IS '备注';
