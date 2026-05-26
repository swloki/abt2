-- ============================================================================
-- Labor Process Dict & Routing — 工序字典、工艺路线、BOM关联
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Labor Process Dict — 工序字典
-- ============================================================================

CREATE TABLE labor_process_dicts (
    id          BIGSERIAL   PRIMARY KEY,
    code        VARCHAR(100) NOT NULL,
    name        VARCHAR(255) NOT NULL,
    description TEXT,
    sort_order  INT         NOT NULL DEFAULT 0,
    operator_id BIGINT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ,
    deleted_at  TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_labor_process_dicts_code ON labor_process_dicts (code) WHERE deleted_at IS NULL;
CREATE INDEX idx_labor_process_dicts_sort_order ON labor_process_dicts (sort_order);

-- ============================================================================
-- 2. Routings — 工艺路线
-- ============================================================================

CREATE TABLE routings (
    id          BIGSERIAL   PRIMARY KEY,
    name        VARCHAR(255) NOT NULL,
    description TEXT,
    operator_id BIGINT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ,
    deleted_at  TIMESTAMPTZ
);

CREATE INDEX idx_routings_name_trgm ON routings USING gin (name gin_trgm_ops);

-- ============================================================================
-- 3. Routing Steps — 工艺路线步骤
-- ============================================================================

CREATE TABLE routing_steps (
    id           BIGSERIAL   PRIMARY KEY,
    routing_id   BIGINT      NOT NULL,
    process_code VARCHAR(100) NOT NULL,
    step_order   INT         NOT NULL,
    is_required  BOOLEAN     NOT NULL DEFAULT TRUE,
    remark       TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_routing_steps_routing_id ON routing_steps (routing_id);
CREATE INDEX idx_routing_steps_routing_process ON routing_steps (routing_id, process_code);

-- ============================================================================
-- 4. BOM-Routing — BOM 与工艺路线关联
-- ============================================================================

CREATE TABLE bom_routings (
    id           BIGSERIAL   PRIMARY KEY,
    product_code VARCHAR(100) NOT NULL,
    routing_id   BIGINT      NOT NULL,
    operator_id  BIGINT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_bom_routings_product_code ON bom_routings (product_code);
CREATE INDEX idx_bom_routings_routing_id ON bom_routings (routing_id);

-- ============================================================================
-- 5. BOM Labor Processes — BOM 劳务工序
-- ============================================================================

CREATE TABLE bom_labor_processes (
    id                    BIGSERIAL     PRIMARY KEY,
    product_code          VARCHAR(100)  NOT NULL,
    labor_process_dict_id BIGINT        NOT NULL,
    process_code          VARCHAR(100),
    name                  VARCHAR(255)  NOT NULL,
    unit_price            NUMERIC(20,4) NOT NULL DEFAULT 0,
    quantity              NUMERIC(18,6) NOT NULL DEFAULT 0,
    sort_order            INT           NOT NULL DEFAULT 0,
    remark                TEXT,
    operator_id           BIGINT,
    created_at            TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ,
    deleted_at            TIMESTAMPTZ
);

CREATE INDEX idx_bom_labor_processes_product_code ON bom_labor_processes (product_code) WHERE deleted_at IS NULL;
CREATE INDEX idx_bom_labor_processes_sort_order ON bom_labor_processes (sort_order);

COMMIT;
