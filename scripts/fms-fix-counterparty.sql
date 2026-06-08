BEGIN;
UPDATE cash_journals SET counterparty_id = 2 WHERE id = 2;
UPDATE cash_journals SET counterparty_id = 7 WHERE id = 3;
UPDATE cash_journals SET counterparty_id = 1 WHERE id = 5;
UPDATE cash_journals SET counterparty_id = 2 WHERE id = 9;
UPDATE cash_journals SET counterparty_id = 7 WHERE id = 11;
UPDATE cash_journals SET counterparty_id = 4 WHERE id = 12;
COMMIT;
