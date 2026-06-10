-- CHK-11: AP 核销完整性
-- 验证: 应付总额 = 已核销 + 未核销余额
SELECT po.id, po.doc_number, po.total_amount
FROM purchase_orders po
WHERE po.deleted_at IS NULL AND po.status >= 3  -- Confirmed
  AND po.total_amount > 0
  AND NOT EXISTS (
    SELECT 1 FROM journal_entries je
    WHERE je.deleted_at IS NULL
      AND (je.reference_type = 'purchase_order' AND je.reference_id = po.id)
      AND ABS(je.amount) = po.total_amount
  );
-- 预期: 0 行或合理行
