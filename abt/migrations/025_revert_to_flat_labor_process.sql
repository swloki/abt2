-- Migration 023: Revert to flat labor process model
-- Drops three-layer tables (from migration 021) and recreates simple bom_labor_process

BEGIN;

-- ============================================================================
-- 1. Drop three-layer tables (reverse dependency order)
-- ============================================================================
DROP TABLE IF EXISTS bom_labor_cost;
DROP TABLE IF EXISTS labor_process_group_member;
DROP TABLE IF EXISTS labor_process_group;
DROP TABLE IF EXISTS labor_process;

-- Remove process_group_id column from bom table
ALTER TABLE bom DROP COLUMN IF EXISTS process_group_id;

-- Drop archived old table if it exists (from migration 021)
DROP TABLE IF EXISTS bom_labor_process_archived;

-- ============================================================================
-- 2. Create flat bom_labor_process table
-- ============================================================================
CREATE TABLE bom_labor_process (
    id BIGSERIAL PRIMARY KEY,
    product_code VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    unit_price DECIMAL(18,6) NOT NULL,
    quantity DECIMAL(18,6) NOT NULL DEFAULT 1,
    sort_order INT NOT NULL DEFAULT 0,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    UNIQUE(product_code, name)
);

CREATE INDEX idx_bom_labor_process_product_code ON bom_labor_process(product_code);

COMMENT ON TABLE bom_labor_process IS 'BOM 人工工序表（按产品管理）';
COMMENT ON COLUMN bom_labor_process.product_code IS '产品编码，关联 BOM 的产品';
COMMENT ON COLUMN bom_labor_process.name IS '工序名称';
COMMENT ON COLUMN bom_labor_process.unit_price IS '工序单价';
COMMENT ON COLUMN bom_labor_process.quantity IS '数量';
COMMENT ON COLUMN bom_labor_process.sort_order IS '排序顺序';
COMMENT ON COLUMN bom_labor_process.remark IS '备注';

COMMIT;
