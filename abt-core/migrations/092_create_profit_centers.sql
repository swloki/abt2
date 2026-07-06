-- ============================================================================
-- 092: Profit Centers — 利润中心主数据
-- 用途：成本核算 P&L 按利润中心归集（target.md #6）。此前 cost_entries.profit_center
--      全为 NULL，利润中心 tab 必空；前端硬编码 6 区域标签。本表提供规范主数据。
-- 关系：department_id REFERENCES departments（可选映射，利润中心可跨部门）。
-- ============================================================================

BEGIN;

CREATE TABLE IF NOT EXISTS profit_centers (
    id            BIGSERIAL    PRIMARY KEY,
    code          VARCHAR(50)  NOT NULL,
    name          VARCHAR(200) NOT NULL,
    department_id BIGINT REFERENCES departments(department_id),
    is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
    operator_id   BIGINT,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ,
    deleted_at    TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_profit_centers_code ON profit_centers (code) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_profit_centers_active ON profit_centers (is_active) WHERE deleted_at IS NULL;

-- 种子：前端 fms_cost_analysis.rs 原硬编码的 6 区域（保持既有展示连续性）
INSERT INTO profit_centers (code, name) VALUES
    ('SW',    '华南区'),
    ('EAST',  '华东区'),
    ('NORTH', '华北区'),
    ('SWEST', '西南区'),
    ('NWEST', '西北区'),
    ('NE',    '东北区')
ON CONFLICT DO NOTHING;

COMMIT;
