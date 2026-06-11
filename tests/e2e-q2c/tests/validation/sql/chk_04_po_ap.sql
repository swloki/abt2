-- CHK-04: PO 金额与应付/日记账一致性
-- 验证1: PO total_amount = SUM(item quantity * unit_price)
-- 验证2: cash_journals 中对应 PO 的日记账金额 = PO total_amount (条件性，如果存在)
-- 返回 0 行 = PASS

-- 验证1: PO header vs items 内部一致性
SELECT 'item_mismatch' AS check_type, po.id, po.doc_number,
       po.total_amount AS po_total,
       COALESCE(SUM(poi.quantity * poi.unit_price), 0) AS calc_total,
       po.total_amount - COALESCE(SUM(poi.quantity * poi.unit_price), 0) AS diff
FROM purchase_orders po
JOIN purchase_order_items poi ON poi.order_id = po.id
WHERE po.deleted_at IS NULL
GROUP BY po.id, po.doc_number, po.total_amount
HAVING ABS(po.total_amount - COALESCE(SUM(poi.quantity * poi.unit_price), 0)) > 0.01

UNION ALL

-- 验证2: 条件性检查 — cash_journals 中已有对应 PO 日记账时，金额应匹配
-- source_type=7 (DocumentType::PurchaseOrder), counterparty_type=2 (Supplier)
SELECT 'ap_mismatch' AS check_type, po.id, po.doc_number,
       po.total_amount AS po_total,
       SUM(cj.amount) AS cj_total,
       po.total_amount - SUM(cj.amount) AS diff
FROM purchase_orders po
JOIN cash_journals cj ON cj.source_id = po.id
    AND cj.source_type = 7       -- DocumentType::PurchaseOrder
    AND cj.counterparty_type = 2 -- CounterpartyType::Supplier
    AND cj.deleted_at IS NULL
WHERE po.deleted_at IS NULL
GROUP BY po.id, po.doc_number, po.total_amount
HAVING ABS(po.total_amount - SUM(cj.amount)) > 0.01;
-- 预期: 0 行返回（PO 金额一致，且应付日记账金额匹配）
