-- BOM 成本权限：为 super_admin 角色添加 BOM_COST:READ
-- 注意：super_admin 在代码层面绕过权限检查，此 seed 仅为数据完整性
-- resource_code 使用小写以匹配现有 role_permissions 数据约定
INSERT INTO role_permissions (role_id, resource_code, action_code)
SELECT r.role_id, 'bom_cost', 'read'
FROM roles r
WHERE r.role_code = 'super_admin'
ON CONFLICT DO NOTHING;
