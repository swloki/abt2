-- CHK-05: 工单用料与 BOM 一致性
-- 验证: 最新 E2E 工单的原材料实际领料 ≈ BOM 标准用量 (偏差 > 10% 才报)
-- 注意: 半成品由子工单生产，不通过领料获取，排除在检查外
WITH current_wo AS (
    SELECT id, product_id, planned_qty, created_at FROM work_orders
    WHERE deleted_at IS NULL
    ORDER BY created_at DESC LIMIT 1
),
bom_usage AS (
    SELECT bn.product_id, bn.product_code, bn.quantity AS bom_qty_per,
           wo.planned_qty,
           bn.quantity * wo.planned_qty AS expected_total
    FROM bom_nodes bn
    JOIN boms b ON bn.bom_id = b.bom_id
    CROSS JOIN current_wo wo
    WHERE b.deleted_at IS NULL
      AND wo.product_id = (SELECT product_id FROM products WHERE product_code = 'PRD-FG-001' AND deleted_at IS NULL LIMIT 1)
      AND bn.product_code LIKE 'PRD-RM-%'
),
actual_usage AS (
    SELECT mr_items.product_id, SUM(mr_items.requested_qty) AS actual_total
    FROM material_requisition_items mr_items
    JOIN material_requisitions mr ON mr_items.requisition_id = mr.id AND mr.deleted_at IS NULL
    WHERE mr.created_at >= (SELECT created_at FROM current_wo)
    GROUP BY mr_items.product_id
)
SELECT bu.product_code, bu.expected_total, COALESCE(au.actual_total, 0) AS actual_total,
       ABS(COALESCE(au.actual_total, 0) - bu.expected_total) / NULLIF(bu.expected_total, 0) * 100 AS deviation_pct
FROM bom_usage bu
LEFT JOIN actual_usage au ON au.product_id = bu.product_id
WHERE ABS(COALESCE(au.actual_total, 0) - bu.expected_total) / NULLIF(bu.expected_total, 0) > 0.1;
-- 预期: 0 行返回（偏差 < 10%）
