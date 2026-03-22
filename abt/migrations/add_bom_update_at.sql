-- 为 BOM 表添加更新时间字段
ALTER TABLE bom ADD COLUMN IF NOT EXISTS update_at TIMESTAMPTZ;

-- 添加注释
COMMENT ON COLUMN bom.update_at IS '更新时间';
