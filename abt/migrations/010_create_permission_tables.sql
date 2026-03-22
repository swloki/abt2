-- +migrate Up
-- 用户表（如果不存在）
CREATE TABLE IF NOT EXISTS users (
    user_id BIGSERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(100),
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_super_admin BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- 角色表
CREATE TABLE IF NOT EXISTS roles (
    role_id BIGSERIAL PRIMARY KEY,
    role_name VARCHAR(100) NOT NULL,
    role_code VARCHAR(50) UNIQUE NOT NULL,
    is_system_role BOOLEAN NOT NULL DEFAULT false,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- 用户-角色关联表
CREATE TABLE IF NOT EXISTS user_roles (
    user_id BIGINT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    role_id BIGINT NOT NULL REFERENCES roles(role_id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, role_id)
);

-- 资源表
CREATE TABLE IF NOT EXISTS resources (
    resource_id BIGSERIAL PRIMARY KEY,
    resource_name VARCHAR(100) NOT NULL,
    resource_code VARCHAR(50) UNIQUE NOT NULL,
    group_name VARCHAR(100),
    sort_order INT DEFAULT 0,
    description TEXT
);

-- 操作表
CREATE TABLE IF NOT EXISTS actions (
    action_code VARCHAR(50) PRIMARY KEY,
    action_name VARCHAR(100) NOT NULL,
    sort_order INT DEFAULT 0,
    description TEXT
);

-- 权限表
CREATE TABLE IF NOT EXISTS permissions (
    permission_id BIGSERIAL PRIMARY KEY,
    permission_name VARCHAR(100) NOT NULL,
    resource_id BIGINT NOT NULL REFERENCES resources(resource_id) ON DELETE CASCADE,
    action_code VARCHAR(50) NOT NULL REFERENCES actions(action_code) ON DELETE CASCADE,
    sort_order INT DEFAULT 0,
    description TEXT,
    UNIQUE(resource_id, action_code)
);

-- 角色权限关联表
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id BIGINT NOT NULL REFERENCES roles(role_id) ON DELETE CASCADE,
    permission_id BIGINT NOT NULL REFERENCES permissions(permission_id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id)
);

-- 权限审计日志表
CREATE TABLE IF NOT EXISTS permission_audit_logs (
    log_id BIGSERIAL PRIMARY KEY,
    operator_id BIGINT NOT NULL REFERENCES users(user_id),
    target_type VARCHAR(20) NOT NULL,
    target_id BIGINT NOT NULL,
    action VARCHAR(50) NOT NULL,
    old_value JSONB,
    new_value JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_user_roles_user ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role ON user_roles(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_role ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_permission_audit_logs_operator ON permission_audit_logs(operator_id);
CREATE INDEX IF NOT EXISTS idx_permission_audit_logs_created ON permission_audit_logs(created_at);

-- 预置数据：系统角色
INSERT INTO roles (role_name, role_code, is_system_role, description) VALUES
('超级管理员', 'super_admin', true, '拥有所有权限'),
('管理员', 'admin', true, '管理用户和基础数据'),
('普通用户', 'user', true, '基础访问权限')
ON CONFLICT (role_code) DO NOTHING;

-- 预置数据：操作
INSERT INTO actions (action_code, action_name, sort_order) VALUES
('read', '读取', 1),
('write', '编辑', 2),
('delete', '删除', 3)
ON CONFLICT (action_code) DO NOTHING;

-- 预置数据：资源
INSERT INTO resources (resource_name, resource_code, group_name, sort_order) VALUES
('产品管理', 'product', '基础数据', 1),
('术语/分类管理', 'term', '基础数据', 2),
('仓库管理', 'warehouse', '库存管理', 3),
('库位管理', 'location', '库存管理', 4),
('库存管理', 'inventory', '库存管理', 5),
('BOM管理', 'bom', '生产管理', 6),
('工序管理', 'labor_process', '生产管理', 7),
('价格管理', 'price', '财务管理', 8),
('Excel导入导出', 'excel', '系统工具', 9),
('用户管理', 'user', '系统管理', 10),
('角色管理', 'role', '系统管理', 11),
('权限管理', 'permission', '系统管理', 12)
ON CONFLICT (resource_code) DO NOTHING;

-- 预置数据：权限（资源 × 操作）
INSERT INTO permissions (permission_name, resource_id, action_code, sort_order)
SELECT
    r.resource_name || '-' || a.action_name,
    r.resource_id,
    a.action_code,
    (r.sort_order * 10 + a.sort_order)
FROM resources r
CROSS JOIN actions a
ON CONFLICT (resource_id, action_code) DO NOTHING;

-- +migrate Down
DROP TABLE IF EXISTS permission_audit_logs;
DROP TABLE IF EXISTS role_permissions;
DROP TABLE IF EXISTS permissions;
DROP TABLE IF EXISTS actions;
DROP TABLE IF EXISTS resources;
DROP TABLE IF EXISTS user_roles;
DROP TABLE IF EXISTS roles;
-- 注意：不删除 users 表，可能被其他模块使用
