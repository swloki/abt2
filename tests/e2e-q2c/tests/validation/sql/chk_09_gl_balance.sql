-- CHK-09: 总账借贷平衡
-- 验证: 所有日记账借方 = 贷方
SELECT 'journal_entries' AS source,
       COALESCE(SUM(CASE WHEN amount > 0 THEN amount ELSE 0 END), 0) AS total_debit,
       COALESCE(SUM(CASE WHEN amount < 0 THEN ABS(amount) ELSE 0 END), 0) AS total_credit
FROM journal_entries WHERE deleted_at IS NULL
HAVING ABS(COALESCE(SUM(CASE WHEN amount > 0 THEN amount ELSE 0 END), 0) -
           COALESCE(SUM(CASE WHEN amount < 0 THEN ABS(amount) ELSE 0 END), 0)) > 0.01;
-- 预期: 0 行返回（借贷平衡）或表不存在
