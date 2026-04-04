-- +migrate Up
BEGIN;

-- 1. Add is_default column to departments
ALTER TABLE departments ADD COLUMN IF NOT EXISTS is_default BOOLEAN NOT NULL DEFAULT false;

-- 2. Partial unique index to enforce exactly one default department
CREATE UNIQUE INDEX IF NOT EXISTS idx_departments_single_default
    ON departments (is_default) WHERE is_default = true;

-- 3. Create department_resource_access junction table
CREATE TABLE IF NOT EXISTS department_resource_access (
    department_id BIGINT NOT NULL REFERENCES departments(department_id) ON DELETE CASCADE,
    resource_code VARCHAR(128) NOT NULL,
    PRIMARY KEY (department_id, resource_code)
);

CREATE INDEX IF NOT EXISTS idx_dra_department ON department_resource_access(department_id);

-- 4. Seed: find or create default department, set is_default = true
INSERT INTO departments (department_name, department_code, description, is_default)
VALUES ('默认部门', 'default', '系统默认部门，未分配部门的用户自动归属此部门', true)
ON CONFLICT (department_code) DO UPDATE SET is_default = true;

-- 5. Seed: insert all 8 business resource codes for ALL existing departments
INSERT INTO department_resource_access (department_id, resource_code)
SELECT d.department_id, res.resource_code
FROM departments d
CROSS JOIN (
    SELECT 'product' AS resource_code UNION ALL
    SELECT 'term' UNION ALL
    SELECT 'bom' UNION ALL
    SELECT 'warehouse' UNION ALL
    SELECT 'location' UNION ALL
    SELECT 'inventory' UNION ALL
    SELECT 'price' UNION ALL
    SELECT 'labor_process'
) res
ON CONFLICT (department_id, resource_code) DO NOTHING;

COMMIT;

-- +migrate Down
BEGIN;

DROP TABLE IF EXISTS department_resource_access;
DROP INDEX IF EXISTS idx_departments_single_default;
ALTER TABLE departments DROP COLUMN IF EXISTS is_default;

COMMIT;
