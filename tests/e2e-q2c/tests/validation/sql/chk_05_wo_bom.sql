-- CHK-05: 工单用料与 BOM 一致性
-- 验证: 实际领料 ≈ BOM 标准用量 (偏差 < 50%)
WITH bom_usage AS (
    SELECT bn.product_id, bn.product_code, bn.quantity AS bom_qty_per,
           wo.planned_qty,
           bn.quantity * wo.planned_qty AS expected_total
    FROM bom_nodes bn
    JOIN boms b ON bn.bom_id = b.bom_id
    JOIN work_orders wo ON wo.product_id = (
        SELECT product_id FROM products WHERE product_code = 'PRD-FG-001' LIMIT 1
    )
    WHERE b.bom_name = '成品A-BOM' AND bn.deleted_at IS NULL
),
actual_usage AS (
    SELECT mr_items.product_id, SUM(mr_items.requested_qty) AS actual_total
    FROM material_requisition_items mr_items
    JOIN material_requisitions mr ON mr_items.requisition_id = mr.id AND mr.deleted_at IS NULL
    WHERE mr_items.deleted_at IS NULL
    GROUP BY mr_items.product_id
)
SELECT bu.product_code, bu.expected_total, COALESCE(au.actual_total, 0) AS actual_total,
       ABS(COALESCE(au.actual_total, 0) - bu.expected_total) / NULLIF(bu.expected_total, 0) * 100 AS deviation_pct
FROM bom_usage bu
LEFT JOIN actual_usage au ON au.product_id = bu.product_id
WHERE ABS(COALESCE(au.actual_total, 0) - bu.expected_total) / NULLIF(bu.expected_total, 0) > 0.5;
-- 预期: 0 行返回（偏差 < 50%）
