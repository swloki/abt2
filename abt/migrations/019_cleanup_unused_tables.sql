-- Migration 019: Cleanup unused tables after simplification
-- Drops archived tables from scoped-roles era and unused normalization tables
-- (role_permissions now uses denormalized resource_code + action_code columns)

BEGIN;

-- Drop archived tables from scoped-roles (migration 018 renamed these)
DROP TABLE IF EXISTS user_department_roles_archived;
DROP TABLE IF EXISTS department_resource_access_archived;

-- Drop unused normalization tables
-- role_permissions uses (role_id, resource_code, action_code) directly,
-- no code references these tables anymore:
DROP TABLE IF EXISTS permissions;
DROP TABLE IF EXISTS resources;
DROP TABLE IF EXISTS actions;

COMMIT;
