-- 036: Products 表结构重新设计
-- 将 product_code/unit 提升为独立列，创建 product_price 历史表，清理 meta

BEGIN;

-- ============================================================================
-- Step 1: 添加新列（nullable，后续回填后再加约束）
-- ============================================================================

ALTER TABLE products ADD COLUMN product_code VARCHAR(255);
ALTER TABLE products ADD COLUMN unit VARCHAR(50);

-- ============================================================================
-- Step 2: 从 meta JSONB 回填数据
-- ============================================================================

UPDATE products SET
    product_code = meta->>'product_code',
    unit = meta->>'unit'
WHERE meta IS NOT NULL;

-- 处理 meta 为 NULL 的行（设置默认值）
UPDATE products SET
    product_code = '',
    unit = ''
WHERE product_code IS NULL;

UPDATE products SET
    unit = ''
WHERE unit IS NULL;

-- ============================================================================
-- Step 2.5: 去重 product_code（重复编码加产品 ID 后缀）
-- ============================================================================

UPDATE products SET product_code = product_code || '-' || product_id::text
WHERE product_id IN (
    SELECT MAX(product_id)
    FROM products
    WHERE product_code IS NOT NULL AND product_code != ''
    GROUP BY product_code
    HAVING COUNT(*) > 1
);

-- ============================================================================
-- Step 3: 添加 NOT NULL 和 UNIQUE 约束
-- ============================================================================

ALTER TABLE products ALTER COLUMN product_code SET NOT NULL;
ALTER TABLE products ALTER COLUMN unit SET NOT NULL;

ALTER TABLE products ADD CONSTRAINT products_product_code_key UNIQUE (product_code);

CREATE INDEX idx_products_product_code ON products(product_code);

-- ============================================================================
-- Step 4: 创建 product_price 历史表
-- ============================================================================

CREATE TABLE product_price (
    id BIGSERIAL PRIMARY KEY,
    product_id BIGINT NOT NULL,
    price DECIMAL(18,4) NOT NULL,
    operator_id BIGINT,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_product_price_product_id ON product_price(product_id);
CREATE INDEX idx_product_price_created_at ON product_price(product_id, created_at DESC);

COMMENT ON TABLE product_price IS '产品价格历史表（最新行即当前价格）';
COMMENT ON COLUMN product_price.id IS '记录ID';
COMMENT ON COLUMN product_price.product_id IS '产品ID';
COMMENT ON COLUMN product_price.price IS '价格';
COMMENT ON COLUMN product_price.operator_id IS '操作人用户ID';
COMMENT ON COLUMN product_price.remark IS '备注';
COMMENT ON COLUMN product_price.created_at IS '创建时间';

-- ============================================================================
-- Step 5: 从 product_price_log 迁移历史价格数据到 product_price
-- ============================================================================

INSERT INTO product_price (product_id, price, operator_id, remark, created_at)
SELECT
    ppl.product_id,
    ppl.new_price,
    ppl.operator_id,
    ppl.remark,
    ppl.created_at
FROM product_price_log ppl
ORDER BY ppl.created_at;

-- 补充：对于 price_log 中没有记录但 meta 中有价格的产品
INSERT INTO product_price (product_id, price, operator_id, remark, created_at)
SELECT
    p.product_id,
    (p.meta->>'price')::decimal,
    NULL,
    '从 meta 迁移',
    NOW()
FROM products p
WHERE p.meta->>'price' IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM product_price pp WHERE pp.product_id = p.product_id
  );

-- ============================================================================
-- Step 6: 清理 meta JSONB（移除已提升和废弃的字段）
-- ============================================================================

UPDATE products SET meta =
    (CASE
        WHEN meta IS NULL THEN '{}'
        ELSE
            jsonb_build_object(
                'specification', COALESCE(meta->>'specification', ''),
                'acquire_channel', COALESCE(meta->>'acquire_channel', ''),
                'old_code', meta->>'old_code'
            )
    END);

-- ============================================================================
-- Step 7: 归档 product_price_log（保留数据但不再使用）
-- ============================================================================

-- 移除 FK 约束（防止归档表的级联删除）
ALTER TABLE product_price_log DROP CONSTRAINT IF EXISTS product_price_log_product_id_fkey;

ALTER TABLE product_price_log RENAME TO product_price_log_archived;

COMMIT;
