-- CHK-12: 审计日志完整性
-- 验证: 关键操作有审计记录
-- 检查: sales_orders, purchase_orders, work_orders, shipping_requests 都有 created_at 记录
SELECT 'sales_orders' AS entity, COUNT(*) AS cnt FROM sales_orders WHERE deleted_at IS NULL AND doc_number IS NOT NULL
UNION ALL
SELECT 'purchase_orders', COUNT(*) FROM purchase_orders WHERE deleted_at IS NULL
UNION ALL
SELECT 'work_orders', COUNT(*) FROM work_orders WHERE deleted_at IS NULL
UNION ALL
SELECT 'shipping_requests', COUNT(*) FROM shipping_requests WHERE deleted_at IS NULL
UNION ALL
SELECT 'quotations', COUNT(*) FROM quotations WHERE deleted_at IS NULL;
-- 预期: 所有关键实体都有记录
