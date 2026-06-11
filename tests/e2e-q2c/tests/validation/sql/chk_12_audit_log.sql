-- CHK-12: 审计日志完整性
-- 验证: 关键实体在 audit_logs 中有 Create(action=1) 操作记录
-- 检查范围: 当天创建的业务记录（确保新代码的审计功能正常工作）
-- 返回 0 行 = PASS
SELECT t.entity_type, t.today_count, COALESCE(a.actual_count, 0) AS actual_count
FROM (
    SELECT 'WorkOrder' AS entity_type,
           (SELECT COUNT(*) FROM work_orders WHERE deleted_at IS NULL AND created_at::date = CURRENT_DATE) AS today_count
    UNION ALL
    SELECT 'SalesOrder',
           (SELECT COUNT(*) FROM sales_orders WHERE deleted_at IS NULL AND created_at::date = CURRENT_DATE)
    UNION ALL
    SELECT 'PurchaseOrder',
           (SELECT COUNT(*) FROM purchase_orders WHERE deleted_at IS NULL AND created_at::date = CURRENT_DATE)
    UNION ALL
    SELECT 'Quotation',
           (SELECT COUNT(*) FROM quotations WHERE deleted_at IS NULL AND created_at::date = CURRENT_DATE)
) t
LEFT JOIN (
    SELECT entity_type, COUNT(DISTINCT entity_id) AS actual_count
    FROM audit_logs
    WHERE action = 1  -- AuditAction::Create
      AND entity_type IN ('SalesOrder', 'PurchaseOrder', 'WorkOrder', 'Quotation')
      AND created_at::date = CURRENT_DATE
    GROUP BY entity_type
) a ON a.entity_type = t.entity_type
WHERE t.today_count > 0
  AND COALESCE(a.actual_count, 0) < t.today_count;
-- 预期: 0 行返回（当天创建的业务记录都有审计日志）
