-- ============================================================================
-- inventory_reservations — warehouse_id 改为可空
-- Database: abt_v2
-- 销售订单确认阶段的库存预留按 product 维度跨仓库 ATP 汇总（设计提案 §3.2.1：
-- 预留当前不绑定具体仓库，warehouse/batch 维度为后期扩展点）。warehouse_id = NULL
-- 表示「跨仓库预留」。fulfill/cancel 均按 source_type/source_id/source_line_id 查询，
-- 不依赖 warehouse_id，故改可空无下游破坏。
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. warehouse_id 改为可空（NULL = 跨仓库预留）
-- ============================================================================
ALTER TABLE inventory_reservations
    ALTER COLUMN warehouse_id DROP NOT NULL;

-- ============================================================================
-- 2. 新增部分索引，覆盖跨仓库（warehouse_id IS NULL）预留的查询
--    原 idx_inv_res_product (product_id, warehouse_id, status) 对 NULL warehouse 行无效
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_inv_res_product_status_null
    ON inventory_reservations (product_id, status)
    WHERE warehouse_id IS NULL;

COMMIT;
