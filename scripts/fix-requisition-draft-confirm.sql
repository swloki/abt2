-- 修复历史批次领料单卡在 Draft（未 confirm），仓库「待领料」看不到
-- =========================================================================
-- 根因：create_for_routing_step 曾创建领料单后不 confirm（Draft），
-- 而 WMS 待领料（work_center/repo.rs Requisition.statuses=&[2]）仅显示 Confirmed，
-- 导致生产侧点的领料单仓库看不到。
--
-- 本脚本：把有 batch_id 的（create_for_routing_step 创建）Draft 领料单 confirm。
-- 手动领料（batch_id=null，create_manual）Draft 保持（用户草稿，需手动确认）。
-- 用法：psql "$DATABASE_URL" -f scripts/fix-requisition-draft-confirm.sql
-- =========================================================================

BEGIN;

UPDATE stock_pickings
SET status = 2  -- Confirmed
WHERE picking_type = 5  -- InternalIssue
  AND status = 1        -- Draft
  AND deleted_at IS NULL
  AND id IN (
    SELECT DISTINCT picking_id FROM stock_picking_items WHERE batch_id IS NOT NULL
  );

-- 验证：剩余 batch Draft 领料单（期望 0）
SELECT COUNT(*) AS remaining_batch_draft
FROM stock_pickings
WHERE picking_type = 5 AND status = 1 AND deleted_at IS NULL
  AND id IN (SELECT DISTINCT picking_id FROM stock_picking_items WHERE batch_id IS NOT NULL);

COMMIT;
