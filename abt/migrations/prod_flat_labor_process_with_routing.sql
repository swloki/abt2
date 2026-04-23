-- ============================================================================
-- 生产环境迁移：labor process 重构 + routing 新增
-- 执行前提：生产库当前有三层模型表（migration 021 状态）
--   - labor_process, labor_process_group, labor_process_group_member, bom_labor_cost
--   - bom_labor_process_archived（原始 flat 表被 rename）
--   - bom.process_group_id 列
-- 执行结果：
--   - 删除三层模型旧表
--   - 重建 bom_labor_process（flat 模型，空表，后续重新导入数据）
--   - 新增 routing, routing_step, labor_process_dict, bom_routing 表
--   - bom_labor_process 增加 process_code 列
--   - 移除 bom.process_group_id 列
-- ============================================================================

BEGIN;

-- ============================================================================
-- PART 1: 清理三层模型，恢复 flat bom_labor_process
-- ============================================================================

-- 1.1 创建 flat bom_labor_process 表（空表，数据后续重新导入）
DROP TABLE IF EXISTS bom_labor_process;

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

-- 1.2 删除归档表（migration 021 的 rename 产物，已无用）
DROP TABLE IF EXISTS bom_labor_process_archived;

-- 1.3 删除三层模型旧表（反向依赖顺序）
DROP TABLE IF EXISTS bom_labor_cost;
DROP TABLE IF EXISTS labor_process_group_member;
DROP TABLE IF EXISTS labor_process_group;
DROP TABLE IF EXISTS labor_process;

-- 1.4 移除 bom 表的 process_group_id 列
ALTER TABLE bom DROP COLUMN IF EXISTS process_group_id;

-- ============================================================================
-- PART 2: 新增 routing 相关表
-- ============================================================================

-- 2.1 先删除 routing 相关旧表（反向依赖顺序）
DROP TABLE IF EXISTS bom_routing;
DROP TABLE IF EXISTS routing_step;
DROP TABLE IF EXISTS routing;
DROP TABLE IF EXISTS labor_process_dict;

-- 2.2 工序字典表
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

-- 2.3 工艺路线表
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

-- 2.4 路线工序明细表
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

-- 2.5 BOM 路线映射表
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

-- 2.6 工序字典编码序列
CREATE SEQUENCE IF NOT EXISTS labor_process_dict_code_seq START WITH 1;

COMMENT ON SEQUENCE labor_process_dict_code_seq IS '工序字典编码自增序列';

-- 2.7 bom_labor_process 增加 process_code 列
ALTER TABLE bom_labor_process ADD COLUMN IF NOT EXISTS process_code VARCHAR(50);

CREATE INDEX idx_bom_labor_process_process_code ON bom_labor_process(process_code) WHERE process_code IS NOT NULL;

COMMENT ON COLUMN bom_labor_process.process_code IS '工序编码，关联 labor_process_dict.code（应用层关联）';

COMMIT;
