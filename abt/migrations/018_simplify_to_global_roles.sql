-- Migration 018: Simplify to Global Roles
-- Remove department-scoped roles, migrate to global user_roles

BEGIN;

-- Step 1: Migrate role assignments from user_department_roles to user_roles (role union)
-- Takes DISTINCT user_id + role_id across all departments.
-- Existing global role assignments in user_roles are preserved (ON CONFLICT DO NOTHING).
INSERT INTO user_roles (user_id, role_id)
SELECT DISTINCT user_id, role_id
FROM user_department_roles
ON CONFLICT (user_id, role_id) DO NOTHING;

-- Step 2: Archive (rename) old tables instead of dropping, for post-deploy verification.
-- Drop manually after confirming the migration is correct:
--   ALTER TABLE user_department_roles_archived DROP TABLE;
--   ALTER TABLE department_resource_access_archived DROP TABLE;
ALTER TABLE user_department_roles RENAME TO user_department_roles_archived;
ALTER TABLE department_resource_access RENAME TO department_resource_access_archived;

COMMIT;
