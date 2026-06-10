-- CHK-04: PO 金额一致性
-- 验证: PO total_amount = SUM(item quantity * unit_price)
SELECT po.id, po.doc_number,
       po.total_amount AS po_total,
       COALESCE(SUM(poi.quantity * poi.unit_price), 0) AS calc_total,
       po.total_amount - COALESCE(SUM(poi.quantity * poi.unit_price), 0) AS diff
FROM purchase_orders po
JOIN purchase_order_items poi ON poi.order_id = po.id
WHERE po.deleted_at IS NULL
GROUP BY po.id, po.doc_number, po.total_amount
HAVING ABS(po.total_amount - COALESCE(SUM(poi.quantity * poi.unit_price), 0)) > 0.01;
-- 预期: 0 行返回
