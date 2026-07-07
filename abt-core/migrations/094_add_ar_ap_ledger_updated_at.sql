-- 094: ar_ap_ledger 补 updated_at 列
-- 065 建表只有 created_at；repo.rs::rewrite_amount_by_source（PO 部分收货重算应付）
-- 的 UPDATE 引用 updated_at = NOW()，但该列从未由任何 migration 创建。
-- sqlx::query 是运行时 SQL（非 query! 宏，不编译期检查列），故编译通过、运行时 500：
--   ERROR: column "updated_at" of relation "ar_ap_ledger" does not exist
-- 补列修复（幂等、非破坏性，DEFAULT NOW() 不影响现有行）。

ALTER TABLE ar_ap_ledger ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
