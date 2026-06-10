-- CHK-12: 审计日志完整性
-- 验证: 关键实体都有记录存在（数量 > 0）
-- 返回数量为 0 的实体 = FAIL
SELECT t.entity, t.cnt FROM (
    SELECT 'sales_orders' AS entity, COUNT(*) AS cnt FROM sales_orders WHERE deleted_at IS NULL AND doc_number IS NOT NULL
    UNION ALL
    SELECT 'purchase_orders', COUNT(*) FROM purchase_orders WHERE deleted_at IS NULL
    UNION ALL
    SELECT 'work_orders', COUNT(*) FROM work_orders WHERE deleted_at IS NULL
    UNION ALL
    SELECT 'quotations', COUNT(*) FROM quotations WHERE deleted_at IS NULL
) t WHERE t.cnt = 0;
-- 预期: 0 行返回（所有关键实体都有记录）
