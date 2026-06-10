-- CHK-10: AR 核销完整性
-- 验证: 应收总额 = 已核销 + 未核销余额
-- 如果没有专门 AR 表，检查 SO 金额与收款日记账的匹配
SELECT so.id, so.doc_number, so.total_amount
FROM sales_orders so
WHERE so.deleted_at IS NULL AND so.status >= 5  -- Shipped or later
  AND so.total_amount > 0
  AND NOT EXISTS (
    SELECT 1 FROM journal_entries je
    WHERE je.deleted_at IS NULL
      AND (je.reference_type = 'sales_order' AND je.reference_id = so.id)
      AND ABS(je.amount) = so.total_amount
  );
-- 预期: 0 行或合理行（取决于 AR 自动生成是否实现）
