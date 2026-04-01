-- +migrate Up
ALTER TABLE permission_audit_logs ALTER COLUMN operator_id DROP NOT NULL;

-- +migrate Down
ALTER TABLE permission_audit_logs ALTER COLUMN operator_id SET NOT NULL;
