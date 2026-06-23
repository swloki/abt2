-- Phase 2：彻底删除 GL + 发票 + expense 模块
-- 配合代码层删除 abt-core/src/gl、fms/expense、发票/expense/GL 相关枚举

BEGIN;

-- 1. 迁移历史台账 source_type：Phase 1 的发票来源 → 业务单据来源
--    SalesInvoice(46) → ShippingRequest(3)；PurchaseInvoice(47) → ArrivalNotice(16)
UPDATE ar_ap_ledger SET source_type = 3  WHERE source_type = 46;
UPDATE ar_ap_ledger SET source_type = 16 WHERE source_type = 47;
UPDATE ar_ap_ledger SET against_type = 3  WHERE against_type = 46;
UPDATE ar_ap_ledger SET against_type = 16 WHERE against_type = 47;
UPDATE ar_ap_settlements SET invoice_source_type = 3  WHERE invoice_source_type = 46;
UPDATE ar_ap_settlements SET invoice_source_type = 16 WHERE invoice_source_type = 47;

-- 2. 删除引用 GL/发票的单据关联（DocumentType 45/46/47 将从枚举移除）
DELETE FROM document_links WHERE source_type IN (45, 46, 47) OR target_type IN (45, 46, 47);

-- 3. drop 发票表（先子后主，外键到 gl_entries 由 CASCADE 处理）
DROP TABLE IF EXISTS sales_invoice_items CASCADE;
DROP TABLE IF EXISTS sales_invoices CASCADE;
DROP TABLE IF EXISTS purchase_invoice_items CASCADE;
DROP TABLE IF EXISTS purchase_invoices CASCADE;

-- 4. drop expense 费用报销表
DROP TABLE IF EXISTS expense_attachments CASCADE;
DROP TABLE IF EXISTS expense_reimbursement_items CASCADE;
DROP TABLE IF EXISTS expense_reimbursements CASCADE;

-- 5. drop 纯 GL 表（先子后主）
DROP TABLE IF EXISTS gl_entry_lines CASCADE;
DROP TABLE IF EXISTS gl_account_mappings CASCADE;
DROP TABLE IF EXISTS gl_entries CASCADE;
DROP TABLE IF EXISTS accounting_periods CASCADE;
DROP TABLE IF EXISTS gl_accounts CASCADE;

-- 6. 清理 GL 权限 seed（058_gl_permissions_seed.sql 注入的）
DELETE FROM role_permissions WHERE resource_code = 'GL';

COMMIT;
