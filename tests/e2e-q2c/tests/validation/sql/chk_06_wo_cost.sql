-- CHK-06: 工单成本归集完整性
-- 验证: 已发料的工单存在材料成本(cost_type=1)分录
-- 只检查有已发料领料单关联的工单（排除未实际领料的历史工单）
-- CostEntityType::WorkOrder = 2, CostType::Material = 1
-- 返回 0 行 = PASS
SELECT wo.id, wo.doc_number, wo.planned_qty,
       COALESCE(ce_mat.total_debit, 0) AS material_debit
FROM work_orders wo
INNER JOIN (
    SELECT DISTINCT work_order_id
    FROM material_requisitions
    WHERE deleted_at IS NULL
      AND status = 3  -- Issued
) mr ON mr.work_order_id = wo.id
LEFT JOIN (
    SELECT entity_id, SUM(debit_amount) AS total_debit
    FROM cost_entries
    WHERE entity_type = 2   -- CostEntityType::WorkOrder
      AND cost_type = 1     -- CostType::Material
    GROUP BY entity_id
) ce_mat ON ce_mat.entity_id = wo.id
WHERE wo.deleted_at IS NULL
  AND (ce_mat.total_debit IS NULL OR ce_mat.total_debit = 0);
-- 预期: 0 行返回（所有已发料工单都有材料成本分录）
