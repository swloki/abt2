-- 037: 回滚 Products 表结构重新设计
-- 恢复 meta JSONB，回写 price，恢复 product_price_log，删除新列

BEGIN;

-- ============================================================================
-- Step 1: 从 product_price 回写当前价格到 meta
-- ============================================================================

UPDATE products p SET meta =
    COALESCE(p.meta, '{}')::jsonb ||
    jsonb_build_object(
        'price', pp.price
    )
FROM (
    SELECT DISTINCT ON (product_id) product_id, price
    FROM product_price
    ORDER BY product_id, created_at DESC
) pp
WHERE p.product_id = pp.product_id;

-- ============================================================================
-- Step 2: 从 product 列回写 product_code/unit/category/subcategory/loss_rate 到 meta
-- 注意：category/subcategory/loss_rate 在前向迁移中已永久移除，
-- 回滚只能恢复 product_code 和 unit
-- ============================================================================

UPDATE products SET meta =
    COALESCE(meta, '{}')::jsonb ||
    jsonb_build_object(
        'product_code', product_code,
        'unit', unit,
        'category', '',
        'subcategory', '',
        'loss_rate', 0
    );

-- ============================================================================
-- Step 3: 恢复 product_price_log 表（从归档表恢复）
-- ============================================================================

ALTER TABLE product_price_log_archived RENAME TO product_price_log;

-- 恢复 FK 约束
ALTER TABLE product_price_log
    ADD CONSTRAINT product_price_log_product_id_fkey
    FOREIGN KEY (product_id) REFERENCES products(product_id) ON DELETE CASCADE;

-- 恢复索引（索引在 RENAME 时保留，但名称可能需要更新）
CREATE INDEX IF NOT EXISTS idx_price_log_product ON product_price_log(product_id);
CREATE INDEX IF NOT EXISTS idx_price_log_created ON product_price_log(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_price_log_operator ON product_price_log(operator_id);

-- ============================================================================
-- Step 4: 删除 product_price 表
-- ============================================================================

DROP TABLE IF EXISTS product_price;

-- ============================================================================
-- Step 5: 删除新列和约束
-- ============================================================================

ALTER TABLE products DROP CONSTRAINT IF EXISTS products_product_code_key;
DROP INDEX IF EXISTS idx_products_product_code;

ALTER TABLE products DROP COLUMN IF EXISTS product_code;
ALTER TABLE products DROP COLUMN IF EXISTS unit;

COMMIT;
