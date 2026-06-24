-- ============================================================================
-- 070. 出纳日记账多币种支持 (issue #69)
-- 新增币种 + 汇率两列；折合人民币金额不落库，按 amount × exchange_rate 动态计算
-- （与 ar_ap_ledger / ar_ap_adjustments 的 currency/exchange_rate 约定一致）
-- 注意：本项目无 migration runner，需手动 psql -f 执行
-- ============================================================================

BEGIN;

ALTER TABLE cash_journals
    ADD COLUMN currency      VARCHAR(10)   NOT NULL DEFAULT 'CNY',
    ADD COLUMN exchange_rate DECIMAL(18,6) NOT NULL DEFAULT 1;

COMMIT;
