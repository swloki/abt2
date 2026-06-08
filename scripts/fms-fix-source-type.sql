-- ============================================================================
-- FMS (Financial Management) Module Test Data — Fix source_type
-- DocumentType: SalesOrder=2, PurchaseOrder=7, ExpenseReimbursement=32,
--               CashJournal=30, WorkOrder=10
-- ============================================================================

BEGIN;

-- Fix source_type in cash_journals (0 is not a valid DocumentType)
UPDATE cash_journals SET source_type = 2  WHERE source_type = 0 AND journal_type = 1;  -- SalesReceipt → SalesOrder
UPDATE cash_journals SET source_type = 7  WHERE source_type = 0 AND journal_type = 2;  -- PurchasePayment → PurchaseOrder
UPDATE cash_journals SET source_type = 32 WHERE source_type = 0 AND journal_type = 3;  -- Expense → ExpenseReimbursement
UPDATE cash_journals SET source_type = 30 WHERE source_type = 0 AND journal_type = 4;  -- Payroll → CashJournal (self)
UPDATE cash_journals SET source_type = 30 WHERE source_type = 0 AND journal_type = 5;  -- Other → CashJournal (self)

-- Also fix source_id for self-referencing ones (payroll, other) — set to their own id
UPDATE cash_journals SET source_id = id WHERE journal_type IN (4, 5);

-- Fix write_offs source_type: use SalesOrder=20? No... let me check the prototype
-- write_offs already has source_type=20 and source_type=30 which are WMS types, let me fix:
UPDATE write_offs SET source_type = 2  WHERE write_off_type = 1;  -- SalesReceipt write-off → SalesOrder
UPDATE write_offs SET source_type = 7  WHERE write_off_type = 2;  -- PurchasePayment write-off → PurchaseOrder

COMMIT;
