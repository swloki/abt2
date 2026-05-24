-- ============================================================================
-- MES Module — Manufacturing Execution System
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- Strictly follows docs/uml-design/04-mes.html entity definitions
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Production Plans — 生产计划
-- ============================================================================

CREATE TABLE production_plans (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(50) NOT NULL,
    plan_date       DATE        NOT NULL,
    plan_type       SMALLINT    NOT NULL,           -- 1=MTO, 2=MTS
    status          SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Confirmed, 3=InProgress, 4=Completed, 5=Cancelled
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    UNIQUE (doc_number)
);

CREATE INDEX idx_production_plans_status ON production_plans (status);
CREATE INDEX idx_production_plans_date ON production_plans (plan_date);

-- ============================================================================
-- 2. Production Plan Items — 生产计划明细
-- ============================================================================

CREATE TABLE production_plan_items (
    id                  BIGSERIAL   PRIMARY KEY,
    plan_id             BIGINT      NOT NULL REFERENCES production_plans(id),
    product_id          BIGINT      NOT NULL,
    planned_qty         DECIMAL(18,6) NOT NULL,
    scheduled_start     DATE        NOT NULL,
    scheduled_end       DATE        NOT NULL,
    sales_order_id      BIGINT,
    sales_order_item_id BIGINT,
    bom_snapshot_id     BIGINT,
    routing_id          BIGINT,
    work_center_id      BIGINT,
    priority            INT         NOT NULL DEFAULT 0,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Planned, 2=Released, 3=InProduction, 4=Completed, 5=Cancelled

    UNIQUE (plan_id, product_id)
);

CREATE INDEX idx_plan_items_plan ON production_plan_items (plan_id);

-- ============================================================================
-- 3. Work Orders — 生产工单
-- ============================================================================

CREATE TABLE work_orders (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(50) NOT NULL,
    plan_item_id    BIGINT      REFERENCES production_plan_items(id),
    product_id      BIGINT      NOT NULL,
    bom_snapshot_id BIGINT,
    routing_id      BIGINT,
    planned_qty     DECIMAL(18,6) NOT NULL,
    scheduled_start DATE        NOT NULL,
    scheduled_end   DATE        NOT NULL,
    status          SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Planned, 3=Released, 4=Closed, 5=Cancelled
    work_center_id  BIGINT,
    sales_order_id  BIGINT,
    version         INT         NOT NULL DEFAULT 1,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    UNIQUE (doc_number) WHERE deleted_at IS NULL
);

CREATE INDEX idx_work_orders_status ON work_orders (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_work_orders_plan_item ON work_orders (plan_item_id) WHERE deleted_at IS NULL;

-- ============================================================================
-- 4. Work Order Routings — 工序（工单级）
-- ============================================================================

CREATE TABLE work_order_routings (
    id                  BIGSERIAL   PRIMARY KEY,
    work_order_id       BIGINT      NOT NULL REFERENCES work_orders(id),
    step_no             INT         NOT NULL,
    process_name        VARCHAR(200) NOT NULL,
    work_center_id      BIGINT,
    standard_time       DECIMAL(18,6),
    standard_cost       DECIMAL(18,6),
    unit_price          DECIMAL(18,6),
    allowed_loss_rate   DECIMAL(18,6),
    planned_qty         DECIMAL(18,6) NOT NULL DEFAULT 0,
    completed_qty       DECIMAL(18,6) NOT NULL DEFAULT 0,
    defect_qty          DECIMAL(18,6) NOT NULL DEFAULT 0,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Pending, 2=InProgress, 3=Completed, 4=Skipped
    is_outsourced       BOOLEAN     NOT NULL DEFAULT false,
    is_inspection_point BOOLEAN     NOT NULL DEFAULT false,

    UNIQUE (work_order_id, step_no)
);

CREATE INDEX idx_routings_work_order ON work_order_routings (work_order_id);

-- ============================================================================
-- 5. Production Batches — 生产批次（流转卡）
-- ============================================================================

CREATE TABLE production_batches (
    id              BIGSERIAL   PRIMARY KEY,
    batch_no        VARCHAR(80) NOT NULL,
    card_sn         VARCHAR(80) NOT NULL,
    work_order_id   BIGINT      NOT NULL REFERENCES work_orders(id),
    product_id      BIGINT      NOT NULL,
    batch_qty       DECIMAL(18,6) NOT NULL,
    completed_qty   DECIMAL(18,6) NOT NULL DEFAULT 0,
    scrap_qty       DECIMAL(18,6) NOT NULL DEFAULT 0,
    team_id         BIGINT,
    current_step    INT         NOT NULL DEFAULT 0,
    actual_start    TIMESTAMPTZ,
    actual_end      TIMESTAMPTZ,
    status          SMALLINT    NOT NULL DEFAULT 1, -- 1=Pending, 2=InProgress, 3=Suspended, 4=PendingReceipt, 5=Completed, 6=Cancelled
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (batch_no),
    UNIQUE (card_sn)
);

CREATE INDEX idx_batches_work_order ON production_batches (work_order_id);
CREATE INDEX idx_batches_status ON production_batches (status);

-- ============================================================================
-- 6. Work Reports — 报工记录
-- ============================================================================

CREATE TABLE work_reports (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(50) NOT NULL,
    work_order_id   BIGINT      NOT NULL REFERENCES work_orders(id),
    batch_id        BIGINT      NOT NULL REFERENCES production_batches(id),
    routing_id      BIGINT      NOT NULL REFERENCES work_order_routings(id),
    report_date     DATE        NOT NULL,
    shift           SMALLINT    NOT NULL,           -- 1=Day, 2=Night
    worker_id       BIGINT      NOT NULL,
    completed_qty   DECIMAL(18,6) NOT NULL,
    defect_qty      DECIMAL(18,6) NOT NULL DEFAULT 0,
    defect_reason   SMALLINT,                       -- 1=MaterialDefect, 2=EquipmentFault, 3=OperatorError, 4=ProcessIssue
    work_hours      DECIMAL(18,6) NOT NULL DEFAULT 0,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number),
    UNIQUE (batch_id, routing_id, worker_id, shift, report_date)
);

CREATE INDEX idx_work_reports_work_order ON work_reports (work_order_id);
CREATE INDEX idx_work_reports_batch ON work_reports (batch_id);
CREATE INDEX idx_work_reports_worker ON work_reports (worker_id, report_date);

-- ============================================================================
-- 7. Production Inspections — 生产报检
-- ============================================================================

CREATE TABLE production_inspections (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(50) NOT NULL,
    work_order_id   BIGINT      NOT NULL REFERENCES work_orders(id),
    routing_id      BIGINT      REFERENCES work_order_routings(id),
    product_id      BIGINT      NOT NULL,
    inspection_type SMALLINT    NOT NULL,           -- 1=FirstArticle, 2=InProcess, 3=Final
    sample_qty      DECIMAL(18,6) NOT NULL DEFAULT 0,
    qualified_qty   DECIMAL(18,6) NOT NULL DEFAULT 0,
    unqualified_qty DECIMAL(18,6) NOT NULL DEFAULT 0,
    result          SMALLINT    NOT NULL DEFAULT 1, -- 1=Pass, 2=Fail, 3=Conditional
    inspector_id    BIGINT      NOT NULL DEFAULT 0,
    inspection_date DATE        NOT NULL,
    disposition     TEXT,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number)
);

CREATE INDEX idx_inspections_work_order ON production_inspections (work_order_id);
CREATE INDEX idx_inspections_product ON production_inspections (product_id);

-- ============================================================================
-- 8. Production Receipts — 完工入库单
-- ============================================================================

CREATE TABLE production_receipts (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    work_order_id       BIGINT      NOT NULL REFERENCES work_orders(id),
    batch_id            BIGINT      REFERENCES production_batches(id),
    product_id          BIGINT      NOT NULL,
    received_qty        DECIMAL(18,6) NOT NULL,
    warehouse_id        BIGINT      NOT NULL,
    zone_id             BIGINT,
    bin_id              BIGINT,
    receipt_date        DATE        NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Confirmed, 3=Cancelled
    backflush_triggered BOOLEAN     NOT NULL DEFAULT false,
    remark              TEXT        NOT NULL DEFAULT '',
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number)
);

CREATE INDEX idx_receipts_work_order ON production_receipts (work_order_id);
CREATE INDEX idx_receipts_batch ON production_receipts (batch_id);

-- ============================================================================
-- 9. State Machine Definitions — MES 状态机配置
-- ============================================================================

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects) VALUES
-- ProductionPlan: Draft -> Confirmed
('production_plan', '1', '2', NULL, NULL, NULL),
-- ProductionPlan: Confirmed -> InProgress
('production_plan', '2', '3', NULL, NULL, NULL),
-- ProductionPlan: InProgress -> Completed
('production_plan', '3', '4', NULL, NULL, NULL),
-- ProductionPlan: Draft -> Cancelled
('production_plan', '1', '5', NULL, NULL, NULL),
-- ProductionPlan: Confirmed -> Cancelled
('production_plan', '2', '5', NULL, NULL, NULL),
-- WorkOrder: Draft -> Planned
('work_order', '1', '2', NULL, NULL, NULL),
-- WorkOrder: Draft -> Released
('work_order', '1', '3', NULL, NULL, NULL),
-- WorkOrder: Planned -> Released
('work_order', '2', '3', NULL, NULL, NULL),
-- WorkOrder: Released -> Closed
('work_order', '3', '4', NULL, NULL, NULL),
-- WorkOrder: Draft -> Cancelled
('work_order', '1', '5', NULL, NULL, NULL),
-- WorkOrder: Planned -> Cancelled
('work_order', '2', '5', NULL, NULL, NULL),
-- WorkOrder: Released -> Cancelled
('work_order', '3', '5', NULL, NULL, NULL),
-- ProductionBatch: Pending -> InProgress
('production_batch', '1', '2', NULL, NULL, NULL),
-- ProductionBatch: InProgress -> InProgress (step advance)
('production_batch', '2', '2', NULL, NULL, NULL),
-- ProductionBatch: InProgress -> Suspended
('production_batch', '2', '3', NULL, NULL, NULL),
-- ProductionBatch: Suspended -> InProgress
('production_batch', '3', '2', NULL, NULL, NULL),
-- ProductionBatch: InProgress -> PendingReceipt
('production_batch', '2', '4', NULL, NULL, NULL),
-- ProductionBatch: PendingReceipt -> Completed
('production_batch', '4', '5', NULL, NULL, NULL),
-- ProductionBatch: Pending -> Cancelled
('production_batch', '1', '6', NULL, NULL, NULL),
-- ProductionBatch: InProgress -> Cancelled
('production_batch', '2', '6', NULL, NULL, NULL),
-- ProductionReceipt: Draft -> Confirmed
('production_receipt', '1', '2', NULL, NULL, NULL),
-- ProductionReceipt: Draft -> Cancelled
('production_receipt', '1', '3', NULL, NULL, NULL);

COMMIT;
