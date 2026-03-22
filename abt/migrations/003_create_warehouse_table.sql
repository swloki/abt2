-- 仓库表
CREATE TABLE IF NOT EXISTS warehouse (
    warehouse_id BIGSERIAL PRIMARY KEY,
    warehouse_name VARCHAR(100) NOT NULL,
    warehouse_code VARCHAR(50) UNIQUE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- 状态索引
CREATE INDEX IF NOT EXISTS idx_warehouse_status ON warehouse(status);

-- 注释
COMMENT ON TABLE warehouse IS '仓库表';
COMMENT ON COLUMN warehouse.warehouse_id IS '仓库ID';
COMMENT ON COLUMN warehouse.warehouse_name IS '仓库名称';
COMMENT ON COLUMN warehouse.warehouse_code IS '仓库编码';
COMMENT ON COLUMN warehouse.status IS '状态: active/inactive';
