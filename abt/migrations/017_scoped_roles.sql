-- ============================================================
-- 017: Scoped Roles Migration
-- Adds: user_department_roles, parent_role_id on roles,
--       manager/staff business roles, data migration
-- ============================================================

-- 1. Add parent_role_id to roles table (for role inheritance)
ALTER TABLE roles ADD COLUMN parent_role_id BIGINT REFERENCES roles(role_id);

-- 2. Create user_department_roles junction table
CREATE TABLE user_department_roles (
    user_id       BIGINT NOT NULL REFERENCES users(user_id),
    department_id BIGINT NOT NULL REFERENCES departments(department_id),
    role_id       BIGINT NOT NULL REFERENCES roles(role_id),
    PRIMARY KEY (user_id, department_id, role_id)
);

CREATE INDEX idx_udr_user_dept ON user_department_roles(user_id, department_id);
CREATE INDEX idx_udr_role ON user_department_roles(role_id);

-- 3. Seed business roles: manager and staff
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES
    ('经理', 'manager', false, '部门经理，拥有业务资源的读写删权限'),
    ('职员', 'staff', false, '普通职员，只有业务资源的只读权限')
ON CONFLICT (role_code) DO NOTHING;

-- 4. Seed manager permissions (all business resources: read + write + delete)
-- manager role_id will be dynamically assigned, use subquery
INSERT INTO role_permissions (role_id, resource_code, action_code)
SELECT r.role_id, v.resource_code, v.action_code
FROM roles r
CROSS JOIN (
    VALUES
        ('product', 'read'), ('product', 'write'), ('product', 'delete'),
        ('term', 'read'), ('term', 'write'), ('term', 'delete'),
        ('bom', 'read'), ('bom', 'write'), ('bom', 'delete'),
        ('warehouse', 'read'), ('warehouse', 'write'), ('warehouse', 'delete'),
        ('location', 'read'), ('location', 'write'), ('location', 'delete'),
        ('inventory', 'read'), ('inventory', 'write'),
        ('price', 'read'), ('price', 'write'),
        ('labor_process', 'read'), ('labor_process', 'write'), ('labor_process', 'delete')
) AS v(resource_code, action_code)
WHERE r.role_code = 'manager'
ON CONFLICT (role_id, resource_code, action_code) DO NOTHING;

-- 5. Seed staff permissions (all business resources: read only)
INSERT INTO role_permissions (role_id, resource_code, action_code)
SELECT r.role_id, v.resource_code, v.action_code
FROM roles r
CROSS JOIN (
    VALUES
        ('product', 'read'),
        ('term', 'read'),
        ('bom', 'read'),
        ('warehouse', 'read'),
        ('location', 'read'),
        ('inventory', 'read'),
        ('price', 'read'),
        ('labor_process', 'read')
) AS v(resource_code, action_code)
WHERE r.role_code = 'staff'
ON CONFLICT (role_id, resource_code, action_code) DO NOTHING;

-- 6. Update system role 'user' permissions (keep user:read, department:read, permission:read)
-- First remove any extra permissions that 'user' role may have
DELETE FROM role_permissions
WHERE role_id = (SELECT role_id FROM roles WHERE role_code = 'user')
  AND NOT (
    (resource_code = 'user' AND action_code = 'read') OR
    (resource_code = 'department' AND action_code = 'read') OR
    (resource_code = 'permission' AND action_code = 'read')
  );

-- 7. Migrate existing user_roles to user_department_roles
--    Assign each user's role to their first department (or default department)
INSERT INTO user_department_roles (user_id, department_id, role_id)
SELECT
    ur.user_id,
    COALESCE(
        (SELECT ud.department_id FROM user_departments ud WHERE ud.user_id = ur.user_id LIMIT 1),
        (SELECT department_id FROM departments WHERE is_default = true LIMIT 1)
    ) AS department_id,
    ur.role_id
FROM user_roles ur
ON CONFLICT (user_id, department_id, role_id) DO NOTHING;

-- 8. Do NOT drop user_roles yet — keep for rollback safety until feature is verified
-- ALTER TABLE user_roles DROP CONSTRAINT IF EXISTS user_roles_pkey;
-- We'll drop it in a follow-up migration once everything is verified
