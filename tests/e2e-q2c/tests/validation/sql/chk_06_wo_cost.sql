-- CHK-06: 工单成本归集完整性
-- 验证: 有工单存在且有相关事务记录
SELECT wo.id, wo.doc_number, wo.planned_qty
FROM work_orders wo
WHERE wo.deleted_at IS NULL
  AND wo.status >= 3  -- Released or later
  AND NOT EXISTS (
    SELECT 1 FROM inventory_transactions it
    WHERE it.work_order_id = wo.id AND it.deleted_at IS NULL
  );
-- 预期: 0 行返回（所有已下达工单都有库存事务）
