BEGIN;

-- 1. Drop old role_permissions (has FK to permissions)
DROP TABLE IF EXISTS role_permissions CASCADE;

-- 2. Drop permissions (has FK to resources and actions)
DROP TABLE IF EXISTS permissions CASCADE;

-- 3. Drop actions table
DROP TABLE IF EXISTS actions CASCADE;

-- 4. Drop resources table
DROP TABLE IF EXISTS resources CASCADE;

-- 5. Create new simplified role_permissions table
CREATE TABLE role_permissions (
    role_id BIGINT NOT NULL REFERENCES roles(role_id) ON DELETE CASCADE,
    resource_code VARCHAR(128) NOT NULL,
    action_code VARCHAR(32) NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, resource_code, action_code)
);

CREATE INDEX idx_role_permissions_role ON role_permissions(role_id);

-- 6. Seed super_admin role with all permissions
INSERT INTO role_permissions (role_id, resource_code, action_code)
SELECT r.role_id, res.resource_code, a.action_code
FROM roles r
CROSS JOIN (
    SELECT 'product' as resource_code UNION ALL
    SELECT 'term' UNION ALL
    SELECT 'bom' UNION ALL
    SELECT 'warehouse' UNION ALL
    SELECT 'location' UNION ALL
    SELECT 'inventory' UNION ALL
    SELECT 'price' UNION ALL
    SELECT 'labor_process' UNION ALL
    SELECT 'excel' UNION ALL
    SELECT 'user' UNION ALL
    SELECT 'role' UNION ALL
    SELECT 'permission' UNION ALL
    SELECT 'department'
) res
CROSS JOIN (
    SELECT 'read' as action_code UNION ALL
    SELECT 'write' UNION ALL
    SELECT 'delete'
) a
WHERE r.role_code = 'super_admin'
ON CONFLICT DO NOTHING;

-- 7. Seed admin role with read+write (no delete on user/role)
INSERT INTO role_permissions (role_id, resource_code, action_code)
SELECT r.role_id, res.resource_code, a.action_code
FROM roles r
CROSS JOIN (
    SELECT 'product' as resource_code UNION ALL
    SELECT 'term' UNION ALL
    SELECT 'bom' UNION ALL
    SELECT 'warehouse' UNION ALL
    SELECT 'location' UNION ALL
    SELECT 'inventory' UNION ALL
    SELECT 'price' UNION ALL
    SELECT 'labor_process' UNION ALL
    SELECT 'excel' UNION ALL
    SELECT 'user' UNION ALL
    SELECT 'role' UNION ALL
    SELECT 'permission' UNION ALL
    SELECT 'department'
) res
CROSS JOIN (
    SELECT 'read' as action_code UNION ALL
    SELECT 'write'
) a
WHERE r.role_code = 'admin'
ON CONFLICT DO NOTHING;

-- 8. Seed user role with basic read permissions
INSERT INTO role_permissions (role_id, resource_code, action_code)
SELECT r.role_id, res.resource_code, 'read' as action_code
FROM roles r
CROSS JOIN (
    SELECT 'product' as resource_code UNION ALL
    SELECT 'term' UNION ALL
    SELECT 'warehouse' UNION ALL
    SELECT 'location' UNION ALL
    SELECT 'inventory' UNION ALL
    SELECT 'bom' UNION ALL
    SELECT 'price'
) res
WHERE r.role_code = 'user'
ON CONFLICT DO NOTHING;

COMMIT;
