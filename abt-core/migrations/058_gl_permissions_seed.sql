-- GL（总账）权限：为 admin 角色（role_code='admin'）授予全部 GL 权限
-- 注：super_admin 用户走 is_super_admin 分支自动 bypass，无需 seed；
--     此 seed 仅为可分配给普通用户的 admin 角色提供 GL 权限模板。

INSERT INTO role_permissions (role_id, resource_code, action)
SELECT r.role_id, 'GL', actions.action
FROM roles r
CROSS JOIN (VALUES ('create'), ('read'), ('update'), ('delete')) AS actions(action)
WHERE r.role_code = 'admin'
ON CONFLICT (role_id, resource_code, action) DO NOTHING;
