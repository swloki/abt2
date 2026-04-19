-- 为 bom 表添加 bom_category_id 列
ALTER TABLE bom
ADD COLUMN IF NOT EXISTS bom_category_id BIGINT REFERENCES bom_category(bom_category_id) ON DELETE SET NULL;

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_bom_category_id ON bom(bom_category_id);
