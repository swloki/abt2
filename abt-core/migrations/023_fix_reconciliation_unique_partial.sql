-- Fix reconciliations unique constraint to exclude soft-deleted rows
BEGIN;

-- Drop the existing non-partial unique constraint
ALTER TABLE reconciliations DROP CONSTRAINT reconciliations_customer_id_period_key;

-- Replace with partial unique index that only covers active rows
CREATE UNIQUE INDEX reconciliations_customer_period_active ON reconciliations (customer_id, period) WHERE deleted_at IS NULL;

COMMIT;
