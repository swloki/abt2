-- Migration 021: Labor Process Redesign
-- Replace bom_labor_process with three-layer model:
--   labor_process (master) → labor_process_group + labor_process_group_member (join) → bom_labor_cost (per-BOM with snapshot)

BEGIN;

-- ============================================================================
-- 1. 劳务工序主表
-- ============================================================================
CREATE TABLE IF NOT EXISTS labor_process (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE,
    unit_price DECIMAL(18,6) NOT NULL,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

COMMENT ON TABLE labor_process IS '劳务工序主表';
COMMENT ON COLUMN labor_process.name IS '工序名称，唯一';
COMMENT ON COLUMN labor_process.unit_price IS '工序单价';
COMMENT ON COLUMN labor_process.remark IS '备注';

-- ============================================================================
-- 2. 工序组
-- ============================================================================
CREATE TABLE IF NOT EXISTS labor_process_group (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

COMMENT ON TABLE labor_process_group IS '劳务工序组';
COMMENT ON COLUMN labor_process_group.name IS '组名称，唯一';
COMMENT ON COLUMN labor_process_group.remark IS '备注';

-- ============================================================================
-- 3. 工序组成员（连接表）
-- ============================================================================
CREATE TABLE IF NOT EXISTS labor_process_group_member (
    group_id BIGINT NOT NULL,
    process_id BIGINT NOT NULL,
    sort_order INT NOT NULL,
    PRIMARY KEY (group_id, process_id)
);

CREATE INDEX IF NOT EXISTS idx_lpgm_group_id ON labor_process_group_member(group_id);
CREATE INDEX IF NOT EXISTS idx_lpgm_process_id ON labor_process_group_member(process_id);

COMMENT ON TABLE labor_process_group_member IS '工序组成员连接表';
COMMENT ON COLUMN labor_process_group_member.group_id IS '工序组 ID';
COMMENT ON COLUMN labor_process_group_member.process_id IS '工序 ID';
COMMENT ON COLUMN labor_process_group_member.sort_order IS '组内排序';

-- ============================================================================
-- 4. BOM 劳务成本
-- ============================================================================
CREATE TABLE IF NOT EXISTS bom_labor_cost (
    id BIGSERIAL PRIMARY KEY,
    bom_id BIGINT NOT NULL,
    process_id BIGINT NOT NULL,
    quantity DECIMAL(18,6) NOT NULL DEFAULT 0,
    unit_price_snapshot DECIMAL(18,6),
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_bom_labor_cost_bom_id ON bom_labor_cost(bom_id);
CREATE INDEX IF NOT EXISTS idx_bom_labor_cost_process_id ON bom_labor_cost(process_id);

COMMENT ON TABLE bom_labor_cost IS 'BOM 劳务成本明细';
COMMENT ON COLUMN bom_labor_cost.bom_id IS '关联的 BOM';
COMMENT ON COLUMN bom_labor_cost.process_id IS '关联的工序';
COMMENT ON COLUMN bom_labor_cost.quantity IS '数量';
COMMENT ON COLUMN bom_labor_cost.unit_price_snapshot IS '设定时冻结的工序单价快照';
COMMENT ON COLUMN bom_labor_cost.remark IS '备注（数量为 0 时必填）';

-- ============================================================================
-- 5. BOM 表增加 process_group_id 列
-- ============================================================================
ALTER TABLE bom ADD COLUMN IF NOT EXISTS process_group_id BIGINT;

COMMENT ON COLUMN bom.process_group_id IS '关联的工序组 ID';

-- ============================================================================
-- 6. 归档旧表
-- ============================================================================
ALTER TABLE IF EXISTS bom_labor_process RENAME TO bom_labor_process_archived;

COMMIT;
