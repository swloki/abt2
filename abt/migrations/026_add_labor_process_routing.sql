-- Migration 026: Add labor process routing tables
-- Creates labor_process_dict, routing, routing_step, bom_routing
-- Adds process_code column to bom_labor_process

BEGIN;

-- ============================================================================
-- 1. 工序字典表
-- ============================================================================
CREATE TABLE labor_process_dict (
    id BIGSERIAL PRIMARY KEY,
    code VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    UNIQUE(code),
    UNIQUE(name)
);

COMMENT ON TABLE labor_process_dict IS '工序字典表（全局工序主数据）';
COMMENT ON COLUMN labor_process_dict.code IS '工序编码（唯一）';
COMMENT ON COLUMN labor_process_dict.name IS '工序名称（唯一）';
COMMENT ON COLUMN labor_process_dict.description IS '说明';
COMMENT ON COLUMN labor_process_dict.sort_order IS '排序';

-- ============================================================================
-- 2. 工艺路线表
-- ============================================================================
CREATE TABLE routing (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

COMMENT ON TABLE routing IS '工艺路线表（可复用的工序组合模板）';
COMMENT ON COLUMN routing.name IS '路线名称';
COMMENT ON COLUMN routing.description IS '说明';

-- ============================================================================
-- 3. 路线工序明细表
-- ============================================================================
CREATE TABLE routing_step (
    id BIGSERIAL PRIMARY KEY,
    routing_id BIGINT NOT NULL,
    process_code VARCHAR(50) NOT NULL,
    step_order INT NOT NULL DEFAULT 0,
    is_required BOOLEAN NOT NULL DEFAULT TRUE,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    UNIQUE(routing_id, process_code)
);

CREATE INDEX idx_routing_step_routing_id ON routing_step(routing_id);

COMMENT ON TABLE routing_step IS '路线工序明细表';
COMMENT ON COLUMN routing_step.routing_id IS '关联路线 ID';
COMMENT ON COLUMN routing_step.process_code IS '关联工序编码';
COMMENT ON COLUMN routing_step.step_order IS '工序顺序';
COMMENT ON COLUMN routing_step.is_required IS '是否必须工序';
COMMENT ON COLUMN routing_step.remark IS '备注';

-- ============================================================================
-- 4. BOM 路线映射表
-- ============================================================================
CREATE TABLE bom_routing (
    id BIGSERIAL PRIMARY KEY,
    product_code VARCHAR(100) NOT NULL,
    routing_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    UNIQUE(product_code)
);

CREATE INDEX idx_bom_routing_product_code ON bom_routing(product_code);
CREATE INDEX idx_bom_routing_routing_id ON bom_routing(routing_id);

COMMENT ON TABLE bom_routing IS 'BOM 路线映射表（产品与工艺路线绑定关系）';
COMMENT ON COLUMN bom_routing.product_code IS '产品编码（唯一）';
COMMENT ON COLUMN bom_routing.routing_id IS '关联路线 ID';

-- ============================================================================
-- 5. 工序字典编码序列
-- ============================================================================
CREATE SEQUENCE IF NOT EXISTS labor_process_dict_code_seq START WITH 1;

COMMENT ON SEQUENCE labor_process_dict_code_seq IS '工序字典编码自增序列';

-- ============================================================================
-- 6. bom_labor_process 增加 process_code 列
-- ============================================================================
ALTER TABLE bom_labor_process ADD COLUMN IF NOT EXISTS process_code VARCHAR(50);

CREATE INDEX idx_bom_labor_process_process_code ON bom_labor_process(process_code) WHERE process_code IS NOT NULL;

COMMENT ON COLUMN bom_labor_process.process_code IS '工序编码，关联 labor_process_dict.code（应用层关联）';

COMMIT;
