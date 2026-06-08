-- 插入 15 条额外日记账（共 27 条，触发分页）
-- source_type=1(CashJournal), source_id=0 (self-generated)
INSERT INTO cash_journals (doc_number, period, journal_type, direction, amount, counterparty_type, counterparty_id, source_type, source_id, transaction_date, remark, status, operator_id, created_at)
SELECT
  'CJ-2026-04-' || LPAD((13 + rn)::text, 5, '0'),
  '2026-04',
  (ARRAY[1,2,3,4])[1 + (rn % 4)],
  CASE WHEN (rn % 4) IN (0,3) THEN 1 ELSE 2 END,
  (5000 + rn * 3700)::numeric,
  CASE WHEN (rn % 3) = 0 THEN 1 WHEN (rn % 3) = 1 THEN 2 ELSE 3 END,
  CASE WHEN (rn % 3) = 0 THEN 1 WHEN (rn % 3) = 1 THEN 2 ELSE 7 END,
  1, 0,
  ('2026-04-' || LPAD((28 - rn)::text, 2, '0'))::date,
  CASE (rn % 4)
    WHEN 0 THEN 'sales receipt test'
    WHEN 1 THEN 'purchase payment test'
    WHEN 2 THEN 'expense reimbursement test'
    ELSE 'payroll payment test'
  END,
  2,
  1,
  NOW()
FROM generate_series(0, 14) AS rn
ON CONFLICT DO NOTHING;

-- 插入 15 条额外报销单（共 21 条，触发分页）
INSERT INTO expense_reimbursements (doc_number, applicant_id, department_id, expense_date, total_amount, status, remark, operator_id, created_at)
SELECT
  'EX-2026-04-' || LPAD((7 + rn)::text, 5, '0'),
  1,
  NULL,
  ('2026-04-' || LPAD((28 - rn)::text, 2, '0'))::date,
  (1000 + rn * 850)::numeric,
  CASE WHEN rn < 12 THEN 3 ELSE 2 END,
  'expense test ' || rn,
  1,
  NOW()
FROM generate_series(0, 14) AS rn
ON CONFLICT DO NOTHING;

-- 为新增报销单添加费用明细 (reimbursement_id not expense_id)
INSERT INTO expense_reimbursement_items (reimbursement_id, expense_type, amount, description, receipt_no)
SELECT
  e.id,
  (ARRAY[1,2,3,4,5])[1 + (rn % 5)],
  e.total_amount,
  'item detail test ' || rn,
  'INV-' || LPAD((rn + 100)::text, 6, '0')
FROM (SELECT id, total_amount, ROW_NUMBER() OVER () - 1 AS rn FROM expense_reimbursements WHERE doc_number LIKE 'EX-2026-04-%') e;
