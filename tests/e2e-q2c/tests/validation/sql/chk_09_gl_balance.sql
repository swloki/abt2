-- CHK-09: 总账借贷平衡
-- 验证: cash_journal_lines 中每个 journal_id 的 debit_amount 合计 = credit_amount 合计
-- 返回 0 行 = PASS
SELECT cjl.journal_id,
       SUM(cjl.debit_amount) AS total_debit,
       SUM(cjl.credit_amount) AS total_credit,
       SUM(cjl.debit_amount) - SUM(cjl.credit_amount) AS diff
FROM cash_journal_lines cjl
JOIN cash_journals cj ON cj.id = cjl.journal_id AND cj.deleted_at IS NULL
GROUP BY cjl.journal_id
HAVING ABS(SUM(cjl.debit_amount) - SUM(cjl.credit_amount)) > 0.01;
-- 预期: 0 行返回（借贷平衡）
