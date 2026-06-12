-- ============================================================================
-- Products Acquire Channel — 产品采购渠道
-- Database: abt_v2
-- 添加采购渠道枚举字段到 products 表，从 meta 字段迁移数据
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. 添加 acquire_channel 列
-- ============================================================================

ALTER TABLE products
ADD COLUMN acquire_channel SMALLINT NOT NULL DEFAULT 9;

-- ============================================================================
-- 2. 添加 CHECK 约束
-- ============================================================================

ALTER TABLE products
ADD CONSTRAINT chk_products_acquire_channel
CHECK (acquire_channel IN (1, 2, 3, 4, 9));

-- ============================================================================
-- 3. 创建部分 B-tree 索引
-- ============================================================================

CREATE INDEX idx_products_acquire_channel ON products (acquire_channel) WHERE deleted_at IS NULL;

-- ============================================================================
-- 4. 数据迁移：从 meta->>'acquire_channel' 映射到枚举值
-- ============================================================================

-- 更新自产产品: 'self-made', '自制', '自产' → 1
UPDATE products
SET acquire_channel = 1
WHERE deleted_at IS NULL
  AND (meta->>'acquire_channel' = 'self-made'
       OR meta->>'acquire_channel' = '自制'
       OR meta->>'acquire_channel' = '自产');

-- 更新外购产品: 'purchase', '外购', '采购' → 2
UPDATE products
SET acquire_channel = 2
WHERE deleted_at IS NULL
  AND (meta->>'acquire_channel' = 'purchase'
       OR meta->>'acquire_channel' = '外购'
       OR meta->>'acquire_channel' = '采购');

-- 更新委外产品: 'outsourced', '委外' → 3
UPDATE products
SET acquire_channel = 3
WHERE deleted_at IS NULL
  AND (meta->>'acquire_channel' = 'outsourced'
       OR meta->>'acquire_channel' = '委外');

-- 更新非库存/服务产品: 'non-inventory', '费用', '服务' → 4
UPDATE products
SET acquire_channel = 4
WHERE deleted_at IS NULL
  AND (meta->>'acquire_channel' = 'non-inventory'
       OR meta->>'acquire_channel' = '费用'
       OR meta->>'acquire_channel' = '服务');

-- 默认值: 所有其他值保持 9 (Legacy)
-- acquire_channel 列已经默认值为 9，无需更新

COMMIT;