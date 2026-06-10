-- 权限测试清理脚本
-- 按依赖关系逆序删除

BEGIN;

-- 1. 用户-角色关联
DELETE FROM user_roles WHERE user_id IN (
    SELECT user_id FROM users WHERE username IN (
        'test_sales', 'test_warehouse', 'test_production',
        'test_guest', 'test_empty', 'test_multi', 'test_inherit'
    )
);

-- 2. 测试用户
DELETE FROM users WHERE username IN (
    'test_sales', 'test_warehouse', 'test_production',
    'test_guest', 'test_empty', 'test_multi', 'test_inherit'
);

-- 3. 测试角色权限
DELETE FROM role_permissions WHERE role_id IN (
    SELECT role_id FROM roles WHERE role_code IN (
        'sales_manager', 'warehouse_keeper', 'production_supervisor',
        'readonly_guest', 'empty_role', 'base_role', 'derived_role'
    )
);

-- 5. viewer 角色权限（测试时添加的 read 权限）
DELETE FROM role_permissions WHERE role_id IN (
    SELECT role_id FROM roles WHERE role_code = 'viewer'
);

-- 6. 测试角色（readonly_guest 需先清除 parent_role_id）
UPDATE roles SET parent_role_id = NULL WHERE role_code = 'readonly_guest';
UPDATE roles SET parent_role_id = NULL WHERE role_code = 'derived_role';

DELETE FROM roles WHERE role_code IN (
    'sales_manager', 'warehouse_keeper', 'production_supervisor',
    'readonly_guest', 'empty_role', 'base_role', 'derived_role'
);

-- 7. viewer 角色（测试时创建的）
DELETE FROM roles WHERE role_code = 'viewer';

-- 8. 测试部门
DELETE FROM departments WHERE department_code IN (
    'SALES', 'WAREHOUSE_DEPT', 'PRODUCTION', 'MANAGEMENT'
);

COMMIT;
