-- 清理重复领料单（同一 batch+routing 多个活跃单，重复点击 bug 产物）
-- =========================================================================
-- 根因：create_for_routing_step 曾无幂等，重复点击生成多张领料单。
-- 已加幂等查（find_active_picking_by_batch_operation）。本脚本清理历史重复：
-- 每个 batch+routing 保留最早 1 张，其余 qty_done=0（未发料）的重复单取消。
-- 已发料（qty_done>0）的重复单保留（库存已动，不安全取消，需人工核销）。
-- 用法：psql "$DATABASE_URL" -f scripts/fix-duplicate-requisitions.sql
-- =========================================================================

BEGIN;

WITH ranked AS (
  SELECT p.id,
    ROW_NUMBER() OVER (PARTITION BY i.batch_id, i.operation_id ORDER BY p.created_at, p.id) AS rn
  FROM stock_picking_items i
  JOIN stock_pickings p ON p.id = i.picking_id
  WHERE i.batch_id IS NOT NULL AND i.operation_id IS NOT NULL
    AND p.picking_type = 5  -- InternalIssue
    AND p.status IN (1, 2)  -- Draft/Confirmed
    AND p.deleted_at IS NULL
    AND i.qty_done = 0      -- 仅未发料的重复单（已发料的保留，人工核销）
)
UPDATE stock_pickings
SET status = 4  -- Cancelled
WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

-- 验证：剩余重复（batch+routing 多活跃单，期望只剩已发料的）
SELECT batch_id, operation_id, COUNT(*) AS remaining_cnt
FROM (
  SELECT i.batch_id, i.operation_id, i.picking_id
  FROM stock_picking_items i
  JOIN stock_pickings p ON p.id = i.picking_id
  WHERE i.batch_id IS NOT NULL AND i.operation_id IS NOT NULL
    AND p.picking_type = 5 AND p.status IN (1, 2) AND p.deleted_at IS NULL
) t GROUP BY batch_id, operation_id HAVING COUNT(*) > 1;

COMMIT;
