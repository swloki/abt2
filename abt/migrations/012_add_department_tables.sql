-- +migrate Up
-- 部门表
CREATE TABLE IF NOT EXISTS departments (
    department_id BIGSERIAL PRIMARY KEY,
    department_name VARCHAR(100) NOT NULL,
    department_code VARCHAR(50) UNIQUE NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- 用户-部门关联表
CREATE TABLE IF NOT EXISTS user_departments (
    user_id BIGINT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    department_id BIGINT NOT NULL REFERENCES departments(department_id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, department_id)
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_user_departments_user ON user_departments(user_id);
CREATE INDEX IF NOT EXISTS idx_user_departments_department ON user_departments(department_id);

-- 在现有 resources 表增加 department_id 字段
ALTER TABLE resources ADD COLUMN IF NOT EXISTS department_id BIGINT REFERENCES departments(department_id);
CREATE INDEX IF NOT EXISTS idx_resources_department ON resources(department_id);

-- +migrate Down
DROP INDEX IF EXISTS idx_resources_department;
ALTER TABLE resources DROP COLUMN IF EXISTS department_id;
DROP TABLE IF EXISTS user_departments;
DROP TABLE IF EXISTS departments;
