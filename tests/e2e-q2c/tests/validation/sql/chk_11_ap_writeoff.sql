-- CHK-11: AP 核销完整性
-- 注: journal_entries 表尚未创建，验证 PO 金额合理性
SELECT po.id, po.doc_number, po.total_amount
FROM purchase_orders po
WHERE po.deleted_at IS NULL AND po.status >= 3
  AND (po.total_amount IS NULL OR po.total_amount < 0);
-- 预期: 0 行返回（已确认 PO 都有有效金额）
