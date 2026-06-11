-- CHK-02: SO 金额与应收/日记账一致性
-- 验证1: SO total_amount = SUM(item amount)
-- 验证2: cash_journals 中对应 SO 的日记账金额 = SO total_amount (条件性，如果存在)
-- 返回 0 行 = PASS

-- 验证1: SO header vs items 内部一致性
SELECT 'item_mismatch' AS check_type, so.id, so.doc_number,
       so.total_amount AS order_total,
       COALESCE(SUM(soi.quantity * soi.unit_price * (1 - COALESCE(soi.discount_rate, 0) / 100.0)), 0) AS calc_total,
       so.total_amount - COALESCE(SUM(soi.quantity * soi.unit_price * (1 - COALESCE(soi.discount_rate, 0) / 100.0)), 0) AS diff
FROM sales_orders so
JOIN sales_order_items soi ON soi.order_id = so.id
WHERE so.deleted_at IS NULL AND so.doc_number LIKE 'SO-%'
GROUP BY so.id, so.doc_number, so.total_amount
HAVING ABS(so.total_amount - COALESCE(SUM(soi.quantity * soi.unit_price * (1 - COALESCE(soi.discount_rate, 0) / 100.0)), 0)) > 0.01

UNION ALL

-- 验证2: 条件性检查 — cash_journals 中已有对应 SO 日记账时，金额应匹配
-- source_type=2 (DocumentType::SalesOrder), counterparty_type=1 (Customer)
SELECT 'ar_mismatch' AS check_type, so.id, so.doc_number,
       so.total_amount AS order_total,
       SUM(cj.amount) AS cj_total,
       so.total_amount - SUM(cj.amount) AS diff
FROM sales_orders so
JOIN cash_journals cj ON cj.source_id = so.id
    AND cj.source_type = 2       -- DocumentType::SalesOrder
    AND cj.counterparty_type = 1 -- CounterpartyType::Customer
    AND cj.deleted_at IS NULL
WHERE so.deleted_at IS NULL AND so.doc_number LIKE 'SO-%'
GROUP BY so.id, so.doc_number, so.total_amount
HAVING ABS(so.total_amount - SUM(cj.amount)) > 0.01;
-- 预期: 0 行返回（SO 金额一致，且应收日记账金额匹配）
