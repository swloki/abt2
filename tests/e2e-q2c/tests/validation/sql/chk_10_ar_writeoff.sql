-- CHK-10: AR 核销完整性
-- 注: journal_entries 表尚未创建，验证 SO 金额合理性
SELECT so.id, so.doc_number, so.total_amount
FROM sales_orders so
WHERE so.deleted_at IS NULL AND so.status >= 5
  AND (so.total_amount IS NULL OR so.total_amount < 0);
-- 预期: 0 行返回（已发货 SO 都有有效金额）
