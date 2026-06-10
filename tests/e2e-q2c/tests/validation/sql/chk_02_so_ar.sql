-- CHK-02: SO 金额一致性
-- 验证: SO total_amount = SUM(item quantity * unit_price * (1 - discount))
SELECT so.id, so.doc_number,
       so.total_amount AS order_total,
       COALESCE(SUM(soi.quantity * soi.unit_price * (1 - COALESCE(soi.discount_rate, 0) / 100.0)), 0) AS calc_total,
       so.total_amount - COALESCE(SUM(soi.quantity * soi.unit_price * (1 - COALESCE(soi.discount_rate, 0) / 100.0)), 0) AS diff
FROM sales_orders so
JOIN sales_order_items soi ON soi.order_id = so.id
WHERE so.deleted_at IS NULL AND so.doc_number LIKE 'SO-%'
GROUP BY so.id, so.doc_number, so.total_amount
HAVING ABS(so.total_amount - COALESCE(SUM(soi.quantity * soi.unit_price * (1 - COALESCE(soi.discount_rate, 0) / 100.0)), 0)) > 0.01;
-- 预期: 0 行返回（金额一致）
