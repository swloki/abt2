-- Outsourcing Management Module (OM)
-- Three tables: outsourcing_orders, outsourcing_materials, outsourcing_trackings

BEGIN;

-- ---------------------------------------------------------------------------
-- outsourcing_orders: 委外单主表
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS outsourcing_orders (
    id              BIGSERIAL PRIMARY KEY,
    doc_number      TEXT        NOT NULL,
    work_order_id   BIGINT,
    routing_id      BIGINT,
    supplier_id     BIGINT      NOT NULL,
    product_id      BIGINT      NOT NULL,
    outsourcing_type SMALLINT    NOT NULL DEFAULT 1,     -- OutsourcingType
    planned_qty     NUMERIC(18,6) NOT NULL DEFAULT 0,
    completed_qty   NUMERIC(18,6) NOT NULL DEFAULT 0,
    unit_price      NUMERIC(18,6) NOT NULL DEFAULT 0,
    scheduled_date  DATE,
    status          SMALLINT    NOT NULL DEFAULT 1,     -- OutsourcingStatus (Draft)
    virtual_warehouse_id BIGINT  NOT NULL,
    version         INT         NOT NULL DEFAULT 1,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_outsourcing_orders_doc_number   ON outsourcing_orders (doc_number);
CREATE INDEX idx_outsourcing_orders_status       ON outsourcing_orders (status);
CREATE INDEX idx_outsourcing_orders_supplier_id  ON outsourcing_orders (supplier_id);
CREATE INDEX idx_outsourcing_orders_work_order   ON outsourcing_orders (work_order_id) WHERE work_order_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- outsourcing_materials: 委外发料明细
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS outsourcing_materials (
    id              BIGSERIAL PRIMARY KEY,
    outsourcing_id  BIGINT      NOT NULL REFERENCES outsourcing_orders(id),
    product_id      BIGINT      NOT NULL,
    planned_qty     NUMERIC(18,6) NOT NULL DEFAULT 0,
    sent_qty        NUMERIC(18,6) NOT NULL DEFAULT 0,
    returned_qty    NUMERIC(18,6) NOT NULL DEFAULT 0,
    unit_cost       NUMERIC(18,6) NOT NULL DEFAULT 0
);

CREATE INDEX idx_outsourcing_materials_order_id ON outsourcing_materials (outsourcing_id);

-- ---------------------------------------------------------------------------
-- outsourcing_trackings: 委外追踪节点
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS outsourcing_trackings (
    id              BIGSERIAL PRIMARY KEY,
    outsourcing_id  BIGINT      NOT NULL REFERENCES outsourcing_orders(id),
    node_type       SMALLINT    NOT NULL,                -- TrackingNodeType
    tracked_at      TIMESTAMPTZ,                                   -- NULL = 已计划未完成，有值 = 已跟踪
    planned_at      TIMESTAMPTZ,
    remark          TEXT,
    operator_id     BIGINT      NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_outsourcing_trackings_order_id  ON outsourcing_trackings (outsourcing_id);
CREATE INDEX idx_outsourcing_trackings_node_type ON outsourcing_trackings (outsourcing_id, node_type);
CREATE INDEX idx_outsourcing_trackings_overdue   ON outsourcing_trackings (planned_at) WHERE planned_at IS NOT NULL;

COMMIT;
