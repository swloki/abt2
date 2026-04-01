-- 部门数据
INSERT INTO departments (department_name, department_code, description) VALUES
('sales', 'sales', 'product sales'),
('customer_service', 'customer_service', 'customer support'),
('rd', 'rd', 'research and development'),
('production', 'production', 'manufacturing')
ON CONFLICT (department_code) DO NOTHING;

-- 用户数据 (password: 123456, hashed with Argon2id)
INSERT INTO users (username, password_hash, display_name, is_active, is_super_admin) VALUES
('admin', '$argon2id$v=19$m=19456,t=2,p=1$jaUpCiZVqgXCcye/RiVLrQ$i+by1aN+7RtTedGdKuVV1qF/RbOyWbG/0+RJ8VxN+BkC2M/Lbaw/QWQp/F1v2+f1yR4ZqD8xMk6F4+eG9Tk+Yf/4fE0w+hfncM0eGVi', 'Admin', true, true),
('zhang_san', '$argon2id$v=19$m=19456,t=2,p=1$PjVGxaY+4e0yVXmPzGZmPnmFx7N6x+UqPTP1cN+rF3WTQpK0xa+k1pDmF0u3fKqDmNTX', 'Zhang San', true, false),
('li_si', '$argon2id$v=19$m=19456,t=2,p=1$q+vBf/GMHNp3qBYFOPVfIdR6u+BfXX1LyHjR7t+ER+VPblbTszB1N6ZlH5fDrB1N6Z6w4LG9Tk+Yf/4fE0w+hfncM0eGVi', 'Li Si', true, false),
('wang_wu', '$argon2id$v=19$m=19456,t=2,p=1$ubVhY2ZlbFR5RzNnY2VyZwK+RV5hxL3x2fE0tNQm5ZR8KRWdqm+ykWMqDQp/F1v2+f1yR4ZqD8xMk6F4+eG9Tk+Yf/4fE0w+hfncM0eGVi', 'Wang Wu', true, false),
('zhao_liu', '$argon2id$v=19$m=19456,t=2,p=1$HcGjLQu8f+b7rUzf7qF9gG2j9u+N+XcGR0Z1AxZXxZ3x2d3fKqDmNTX', 'Zhao Liu', true, false)
ON CONFLICT (username) DO NOTHING;

-- 角色数据
INSERT INTO roles (role_name, role_code, is_system_role, description) VALUES
('super_admin', 'super_admin', true, 'all permissions'),
('admin', 'admin', true, 'admin role'),
('user', 'user', true, 'normal user')
ON CONFLICT (role_code) DO NOTHING;

-- 操作数据
INSERT INTO actions (action_code, action_name, sort_order) VALUES
('read', 'read', 1),
('write', 'write', 2),
('delete', 'delete', 3)
ON CONFLICT (action_code) DO NOTHING;

-- 资源数据
INSERT INTO resources (resource_name, resource_code, group_name, sort_order) VALUES
('product', 'product', 'basic', 1),
('term', 'term', 'basic', 2),
('warehouse', 'warehouse', 'inventory', 3),
('location', 'location', 'inventory', 4),
('inventory', 'inventory', 'inventory', 5),
('bom', 'bom', 'production', 6),
('labor_process', 'labor_process', 'production', 7),
('price', 'price', 'finance', 8),
('excel', 'excel', 'tools', 9),
('user', 'user', 'system', 10),
('role', 'role', 'system', 11),
('permission', 'permission', 'system', 12)
ON CONFLICT (resource_code) DO NOTHING;

-- 权限数据 (自动生成)
INSERT INTO permissions (permission_name, resource_id, action_code, sort_order)
SELECT r.resource_name || '-' || a.action_name, r.resource_id, a.action_code, (r.sort_order * 10 + a.sort_order)
FROM resources r
CROSS JOIN actions a
ON CONFLICT (resource_id, action_code) DO NOTHING;
