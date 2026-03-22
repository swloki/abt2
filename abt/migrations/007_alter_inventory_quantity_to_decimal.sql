-- 将库存相关的数量字段从 BIGINT 改为 DECIMAL 以支持小数

-- 修改 inventory 表
ALTER TABLE inventory 
    ALTER COLUMN quantity TYPE DECIMAL(18, 4),
    ALTER COLUMN safety_stock TYPE DECIMAL(18, 4);

-- 修改 inventory_log 表
ALTER TABLE inventory_log
    ALTER COLUMN change_qty TYPE DECIMAL(18, 4),
    ALTER COLUMN before_qty TYPE DECIMAL(18, 4),
    ALTER COLUMN after_qty TYPE DECIMAL(18, 4);

-- 注释
COMMENT ON COLUMN inventory.quantity IS '库存数量（支持小数）';
COMMENT ON COLUMN inventory.safety_stock IS '安全库存（预警阈值，支持小数）';
COMMENT ON COLUMN inventory_log.change_qty IS '变动数量（正数入库，负数出库，支持小数）';
COMMENT ON COLUMN inventory_log.before_qty IS '变动前数量（支持小数）';
COMMENT ON COLUMN inventory_log.after_qty IS '变动后数量（支持小数）';