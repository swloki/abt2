-- CHK-06: 工单成本归集完整性
-- 验证: 所有已下达工单存在且有合理的计划数量
SELECT wo.id, wo.doc_number, wo.planned_qty
FROM work_orders wo
WHERE wo.deleted_at IS NULL
  AND wo.status >= 3  -- Released or later
  AND (wo.planned_qty IS NULL OR wo.planned_qty <= 0);
-- 预期: 0 行返回（所有已下达工单都有有效计划数量）
