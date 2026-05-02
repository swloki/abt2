DROP INDEX IF EXISTS idx_bom_created_by;
DROP INDEX IF EXISTS idx_bom_status;
ALTER TABLE bom DROP COLUMN published_by;
ALTER TABLE bom DROP COLUMN published_at;
ALTER TABLE bom DROP CONSTRAINT bom_status_check;
ALTER TABLE bom DROP COLUMN status;
ALTER TABLE bom DROP COLUMN created_by;
