-- ============================================================================
-- Products & Price Log — 产品与价格日志
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Products — 产品
-- ============================================================================

CREATE TABLE products (
    product_id          BIGSERIAL   PRIMARY KEY,
    pdt_name            VARCHAR(255) NOT NULL,
    product_code        VARCHAR(100) NOT NULL,
    unit                VARCHAR(50)  NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Active, 2=Inactive, 3=Obsolete
    external_code       VARCHAR(100),
    owner_department_id BIGINT,
    meta                JSONB       NOT NULL DEFAULT '{"specification":"","acquire_channel":""}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ,
    deleted_at          TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_products_product_code ON products (product_code) WHERE deleted_at IS NULL;
CREATE INDEX idx_products_status ON products (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_products_pdt_name ON products USING gin (pdt_name gin_trgm_ops);
CREATE INDEX idx_products_product_code_trgm ON products USING gin (product_code gin_trgm_ops);
CREATE INDEX idx_products_owner_department ON products (owner_department_id) WHERE deleted_at IS NULL;

-- ============================================================================
-- 2. Price Log — 价格日志
-- ============================================================================

CREATE TABLE price_log (
    log_id      BIGSERIAL      PRIMARY KEY,
    product_id  BIGINT         NOT NULL,
    price_type  SMALLINT       NOT NULL,           -- 1=Purchase, 2=Sales, 3=StandardCost
    old_price   NUMERIC(20,4),
    new_price   NUMERIC(20,4)  NOT NULL,
    operator_id BIGINT,
    remark      TEXT           NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_price_log_product_type ON price_log (product_id, price_type, created_at DESC);

COMMIT;
