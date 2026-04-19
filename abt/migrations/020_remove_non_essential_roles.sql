-- Migration 020: Remove admin role
-- Keep super_admin, user, and any custom roles

BEGIN;

-- Delete role_permissions for admin role
DELETE FROM role_permissions
WHERE role_id IN (
    SELECT role_id FROM roles WHERE role_code = 'admin'
);

-- Delete user_roles for admin role
DELETE FROM user_roles
WHERE role_id IN (
    SELECT role_id FROM roles WHERE role_code = 'admin'
);

-- Delete the admin role
DELETE FROM roles WHERE role_code = 'admin';

COMMIT;
