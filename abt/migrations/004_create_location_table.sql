-- 库位表
CREATE TABLE IF NOT EXISTS location (
    location_id BIGSERIAL PRIMARY KEY,
    warehouse_id BIGINT NOT NULL,
    location_code VARCHAR(50) NOT NULL,
    location_name VARCHAR(100),
    capacity INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(warehouse_id, location_code)
);

-- 仓库索引
CREATE INDEX IF NOT EXISTS idx_location_warehouse ON location(warehouse_id);

-- 注释
COMMENT ON TABLE location IS '库位表';
COMMENT ON COLUMN location.location_id IS '库位ID';
COMMENT ON COLUMN location.warehouse_id IS '仓库ID (关联 warehouse.warehouse_id)';
COMMENT ON COLUMN location.location_code IS '库位编码 如 A-01-02';
COMMENT ON COLUMN location.location_name IS '库位名称';
COMMENT ON COLUMN location.capacity IS '容量限制（可选）';
