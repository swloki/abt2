-- 库位表添加状态字段
ALTER TABLE location ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'active';

-- 状态索引
CREATE INDEX IF NOT EXISTS idx_location_status ON location(status);

-- 注释
COMMENT ON COLUMN location.status IS '状态: active/inactive';
