-- 产品分类表（层级结构，物化路径）
CREATE TABLE IF NOT EXISTS categories (
    category_id   BIGSERIAL PRIMARY KEY,
    category_name VARCHAR(200) NOT NULL,
    parent_id     BIGINT NOT NULL DEFAULT 0,
    path          VARCHAR(1000) NOT NULL DEFAULT '/',
    meta          JSONB NOT NULL DEFAULT '{"count":0}'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_categories_parent ON categories(parent_id);
CREATE INDEX IF NOT EXISTS idx_categories_path ON categories(path);
CREATE UNIQUE INDEX IF NOT EXISTS idx_categories_name_parent ON categories(category_name, parent_id);

-- 产品-分类关联表（多对多）
CREATE TABLE IF NOT EXISTS product_categories (
    product_id  BIGINT NOT NULL,
    category_id BIGINT NOT NULL REFERENCES categories(category_id) ON DELETE CASCADE,
    PRIMARY KEY (product_id, category_id)
);
