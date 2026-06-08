-- Fix profit center admin expense: change cost_type from 1 to 5
DELETE FROM cost_entries WHERE entity_type=4 AND cost_type=1 AND debit_amount IN (28600, 21800, 18400);

INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(4, 1, 5, 28600.00, 0, 1, 1, '2026-06', 15, 1),
(4, 2, 5, 21800.00, 0, 2, 2, '2026-06', 15, 2),
(4, 3, 5, 18400.00, 0, 3, 3, '2026-06', 15, 3);
