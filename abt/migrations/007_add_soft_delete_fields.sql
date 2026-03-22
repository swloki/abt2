-- 添加软删除字段
-- 为 warehouse 和 location 表添加 deleted_at 字段支持软删除

-- 仓库表添加软删除字段
ALTER TABLE warehouse ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ DEFAULT NULL;

-- 库位表添加软删除字段
ALTER TABLE location ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ DEFAULT NULL;

-- 创建部分索引优化查询（仅索引未删除的记录）
CREATE INDEX IF NOT EXISTS idx_warehouse_not_deleted ON warehouse(warehouse_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_location_not_deleted ON location(location_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_location_warehouse_not_deleted ON location(warehouse_id) WHERE deleted_at IS NULL;
