-- ============================================================================
-- Products — 清理 meta 中残留的 acquire_channel 键
-- Database: abt_v2
-- acquire_channel 已是独立 SMALLINT 列（迁移 032），meta 中的同名键为冗余死键，彻底移除。
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. 清理存量：删除所有行 meta 中的 acquire_channel 键（JSONB '-' 操作符）
-- ============================================================================

UPDATE products
SET meta = meta - 'acquire_channel'
WHERE meta ? 'acquire_channel';

-- ============================================================================
-- 2. 改 DEFAULT，去掉 acquire_channel 键，对齐 ProductMeta(specification/old_code/remark)
-- ============================================================================

ALTER TABLE products
ALTER COLUMN meta
SET DEFAULT '{"specification":"","old_code":null,"remark":null}'::jsonb;

COMMIT;
