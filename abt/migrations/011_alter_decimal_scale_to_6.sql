-- 将所有数量和价格相关字段的小数位从 4 改为 6

-- 修改 inventory 表
ALTER TABLE inventory
    ALTER COLUMN quantity TYPE DECIMAL(18, 6),
    ALTER COLUMN safety_stock TYPE DECIMAL(18, 6);

-- 修改 inventory_log 表
ALTER TABLE inventory_log
    ALTER COLUMN change_qty TYPE DECIMAL(18, 6),
    ALTER COLUMN before_qty TYPE DECIMAL(18, 6),
    ALTER COLUMN after_qty TYPE DECIMAL(18, 6);

-- 修改 product_price_log 表
ALTER TABLE product_price_log
    ALTER COLUMN old_price TYPE DECIMAL(18, 6),
    ALTER COLUMN new_price TYPE DECIMAL(18, 6);

-- 修改 bom_labor_process 表
ALTER TABLE bom_labor_process
    ALTER COLUMN unit_price TYPE DECIMAL(18, 6),
    ALTER COLUMN quantity TYPE DECIMAL(18, 6);

-- 更新注释
COMMENT ON COLUMN inventory.quantity IS '库存数量（支持6位小数）';
COMMENT ON COLUMN inventory.safety_stock IS '安全库存（预警阈值，支持6位小数）';
COMMENT ON COLUMN inventory_log.change_qty IS '变动数量（正数入库，负数出库，支持6位小数）';
COMMENT ON COLUMN inventory_log.before_qty IS '变动前数量（支持6位小数）';
COMMENT ON COLUMN inventory_log.after_qty IS '变动后数量（支持6位小数）';
COMMENT ON COLUMN product_price_log.old_price IS '变动前价格（支持6位小数）';
COMMENT ON COLUMN product_price_log.new_price IS '变动后价格（支持6位小数）';
COMMENT ON COLUMN bom_labor_process.unit_price IS '工序单价（支持6位小数）';
COMMENT ON COLUMN bom_labor_process.quantity IS '数量（支持6位小数）';
