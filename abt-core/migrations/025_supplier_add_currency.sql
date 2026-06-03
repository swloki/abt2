-- ============================================================================
-- Add currency column to suppliers table
-- ISO 4217 three-letter code, default CNY
-- ============================================================================

BEGIN;

ALTER TABLE suppliers
    ADD COLUMN currency VARCHAR(3) NOT NULL DEFAULT 'CNY';

COMMIT;
