-- 权限测试种子数据
-- 前置：admin 用户和 super_admin/admin/viewer 系统角色已存在
-- 密码: test1234 (argon2id)

BEGIN;

-- ============================================================
-- 1. 部门
-- ============================================================
INSERT INTO departments (department_name, department_code, description, is_active, is_default)
VALUES
    ('销售部', 'SALES', '销售团队', true, false),
    ('仓储部', 'WAREHOUSE_DEPT', '仓库管理', true, false),
    ('生产部', 'PRODUCTION', '生产制造', true, false),
    ('管理层', 'MANAGEMENT', '高层管理', true, false)
ON CONFLICT (department_code) DO NOTHING;

-- ============================================================
-- 2. 系统角色（如不存在则创建）
-- ============================================================

-- Viewer 角色（只读访问）
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Viewer', 'viewer', true, 'Read-only access')
ON CONFLICT (role_code) DO NOTHING;

-- ============================================================
-- 3. 业务角色
-- ============================================================

-- 销售经理
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('销售经理', 'sales_manager', false, '销售管理全流程')
ON CONFLICT (role_code) DO NOTHING;

-- 仓管员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('仓管员', 'warehouse_keeper', false, '库存仓储全流程')
ON CONFLICT (role_code) DO NOTHING;

-- 生产主管
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('生产主管', 'production_supervisor', false, '生产管理（无删除权限）')
ON CONFLICT (role_code) DO NOTHING;

-- 只读访客（继承 viewer）
INSERT INTO roles (role_name, role_code, is_system_role, parent_role_id, description)
VALUES ('只读访客', 'readonly_guest', false,
    (SELECT role_id FROM roles WHERE role_code = 'viewer'),
    '继承 viewer 的只读权限')
ON CONFLICT (role_code) DO NOTHING;

-- 空权限角色
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('空权限角色', 'empty_role', false, '边界测试：零权限')
ON CONFLICT (role_code) DO NOTHING;

-- 基础角色（继承链中间层）
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('基础角色', 'base_role', false, '继承链测试：基础层')
ON CONFLICT (role_code) DO NOTHING;

-- 派生角色（继承 base_role）
INSERT INTO roles (role_name, role_code, is_system_role, parent_role_id, description)
VALUES ('派生角色', 'derived_role', false,
    (SELECT role_id FROM roles WHERE role_code = 'base_role'),
    '继承链测试：派生层')
ON CONFLICT (role_code) DO NOTHING;

-- ============================================================
-- 3. 角色-权限分配
-- ============================================================

-- 3.1 viewer 角色权限补充（所有 read）
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

-- 3.2 销售经理: CUSTOMER/SALES_ORDER/SHIPPING CRUD + PRODUCT/CATEGORY/PRICE read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CATEGORY', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'PRICE', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.3 仓管员: WAREHOUSE/LOCATION/INVENTORY CRUD + PRODUCT/CATEGORY read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'CATEGORY', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.4 生产主管: WORK_ORDER/INSPECTION CRU + LABOR_COST RU + COST/PRODUCT/BOM read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'WORK_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'WORK_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'WORK_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'INSPECTION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'INSPECTION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'INSPECTION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'LABOR_COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'LABOR_COST', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'BOM', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.5 基础角色: PRODUCT/CATEGORY read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'base_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'base_role'), 'CATEGORY', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.6 派生角色自身: PRODUCT create（继承 base_role 的 read）
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'derived_role'), 'PRODUCT', 'create')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- ============================================================
-- 4. 测试用户（密码: test1234）
-- ============================================================

INSERT INTO users (username, password_hash, display_name, is_super_admin, is_active) VALUES
    ('test_sales',       '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', '测试-销售经理', false, true),
    ('test_warehouse',   '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', '测试-仓管员',   false, true),
    ('test_production',  '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', '测试-生产主管', false, true),
    ('test_guest',       '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', '测试-只读访客', false, true),
    ('test_empty',       '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', '测试-空权限',   false, true),
    ('test_multi',       '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', '测试-多角色',   false, true),
    ('test_inherit',     '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', '测试-继承链',   false, true)
ON CONFLICT (username) DO NOTHING;

-- ============================================================
-- 5. 用户-角色关联
-- ============================================================

-- test_sales → sales_manager
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'test_sales' AND r.role_code = 'sales_manager'
ON CONFLICT DO NOTHING;

-- test_warehouse → warehouse_keeper
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'test_warehouse' AND r.role_code = 'warehouse_keeper'
ON CONFLICT DO NOTHING;

-- test_production → production_supervisor
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'test_production' AND r.role_code = 'production_supervisor'
ON CONFLICT DO NOTHING;

-- test_guest → readonly_guest
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'test_guest' AND r.role_code = 'readonly_guest'
ON CONFLICT DO NOTHING;

-- test_empty → empty_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'test_empty' AND r.role_code = 'empty_role'
ON CONFLICT DO NOTHING;

-- test_multi → sales_manager + warehouse_keeper
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE (u.username = 'test_multi' AND r.role_code = 'sales_manager')
   OR (u.username = 'test_multi' AND r.role_code = 'warehouse_keeper')
ON CONFLICT DO NOTHING;

-- test_inherit → derived_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'test_inherit' AND r.role_code = 'derived_role'
ON CONFLICT DO NOTHING;

COMMIT;
