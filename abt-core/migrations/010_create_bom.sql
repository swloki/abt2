-- ============================================================================
-- BOM — 物料清单 (主表、节点、快照、分类)
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. BOM Categories — BOM 分类
-- ============================================================================

CREATE TABLE bom_categories (
    bom_category_id   BIGSERIAL   PRIMARY KEY,
    bom_category_name VARCHAR(255) NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- 2. BOMs — 物料清单主表
-- ============================================================================

CREATE TABLE boms (
    bom_id         BIGSERIAL   PRIMARY KEY,
    bom_name       VARCHAR(255) NOT NULL,
    create_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    update_at      TIMESTAMPTZ,
    bom_detail     JSONB       NOT NULL DEFAULT '{"nodes":[]}',
    bom_category_id BIGINT,
    status         SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Published
    version        INT         NOT NULL DEFAULT 1,
    published_at   TIMESTAMPTZ,
    created_by     BIGINT
);

CREATE INDEX idx_boms_status ON boms (status);
CREATE INDEX idx_boms_bom_category_id ON boms (bom_category_id);
CREATE INDEX idx_boms_bom_name ON boms USING gin (bom_name gin_trgm_ops);

-- ============================================================================
-- 3. BOM Nodes — BOM 节点 (树结构)
-- ============================================================================

CREATE TABLE bom_nodes (
    node_id      BIGSERIAL     PRIMARY KEY,
    bom_id       BIGINT        NOT NULL,
    product_id   BIGINT        NOT NULL,
    product_code VARCHAR(100),
    quantity     NUMERIC(18,6) NOT NULL DEFAULT 0,
    parent_id    BIGINT        NOT NULL DEFAULT 0,
    loss_rate    NUMERIC(10,4) NOT NULL DEFAULT 0,
    order_num    INT           NOT NULL DEFAULT 0,
    unit         VARCHAR(50),
    remark       TEXT,
    position     VARCHAR(100),
    work_center  VARCHAR(100),
    properties   TEXT
);

CREATE INDEX idx_bom_nodes_bom_id ON bom_nodes (bom_id);
CREATE INDEX idx_bom_nodes_bom_id_parent ON bom_nodes (bom_id, parent_id);
CREATE INDEX idx_bom_nodes_bom_id_product ON bom_nodes (bom_id, product_id);
CREATE INDEX idx_bom_nodes_parent_id ON bom_nodes (parent_id);

-- ============================================================================
-- 4. BOM Snapshots — BOM 快照 (已发布版本)
-- ============================================================================

CREATE TABLE bom_snapshots (
    snapshot_id  BIGSERIAL   PRIMARY KEY,
    bom_id       BIGINT      NOT NULL,
    version      INT         NOT NULL,
    bom_name     VARCHAR(255) NOT NULL,
    bom_detail   JSONB       NOT NULL,
    published_at TIMESTAMPTZ NOT NULL,
    published_by BIGINT      NOT NULL
);

CREATE INDEX idx_bom_snapshots_bom_version ON bom_snapshots (bom_id, version DESC);

COMMIT;
