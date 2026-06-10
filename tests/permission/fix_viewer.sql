-- Fix: create viewer role + assign read permissions + link readonly_guest
BEGIN;

INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Viewer', 'viewer', true, 'Read-only access')
ON CONFLICT (role_code) DO NOTHING;

INSERT INTO role_permissions (role_id, resource_code, action)
SELECT r.role_id, v.resource_code, 'read'
FROM roles r
CROSS JOIN (VALUES
    ('CUSTOMER'), ('PRODUCT'), ('CATEGORY'), ('BOM'), ('BOM_CATEGORY'),
    ('WAREHOUSE'), ('LOCATION'), ('INVENTORY'), ('PRICE'),
    ('SALES_ORDER'), ('PURCHASE_ORDER'), ('WORK_ORDER'),
    ('INSPECTION'), ('COST'), ('LABOR_COST'),
    ('USER'), ('ROLE'), ('DEPARTMENT'), ('SHIPPING'), ('FMS')
) v(resource_code)
WHERE r.role_code = 'viewer'
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

UPDATE roles SET parent_role_id = (SELECT role_id FROM roles WHERE role_code = 'viewer')
WHERE role_code = 'readonly_guest';

COMMIT;
