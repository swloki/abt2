-- Phase 1: 砍 GL —— 删除 ar_ap_ledger 与 GL 的关联列
-- 配合代码层去掉 account_id / gl_entry_id 字段（往来台账不再需要科目维度与凭证关联）

-- 1. 删除引用 GL 的索引
DROP INDEX IF EXISTS idx_aal_gl;

-- 2. 删除 ar_ap_ledger.account_id (NOT NULL, REFERENCES gl_accounts) 及 gl_entry_id (REFERENCES gl_entries)
ALTER TABLE ar_ap_ledger DROP COLUMN IF EXISTS account_id;
ALTER TABLE ar_ap_ledger DROP COLUMN IF EXISTS gl_entry_id;

-- 注：sales_invoices.gl_entry_id / purchase_invoices.gl_entry_id 及其索引暂保留 ——
--     invoice.cancel() 仍读 inv.gl_entry_id（Phase 1 后恒为 None，自动跳过 GL cancel），
--     物理删表列留 Phase 2，避免本次 migration 牵连 invoice 表结构与 repo。
