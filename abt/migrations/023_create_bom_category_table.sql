-- 创建 BOM 分类表
CREATE TABLE IF NOT EXISTS bom_category (
    bom_category_id BIGSERIAL PRIMARY KEY,
    bom_category_name VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_bom_category_name ON bom_category(bom_category_name);
