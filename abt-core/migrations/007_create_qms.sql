-- ============================================================================
-- QMS Module — Quality Management System
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- Strictly follows docs/uml-design/06-qms.html v2.3 entity definitions
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Inspection Specifications — 检验规格
-- ============================================================================

CREATE TABLE inspection_specifications (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(30) NOT NULL,
    product_id      BIGINT      NOT NULL,
    inspection_type SMALLINT    NOT NULL,           -- 1=IQC, 2=IPQC, 3=FQC, 4=OQC
    check_items     JSONB       NOT NULL DEFAULT '[]',
    sample_plan     JSONB       NOT NULL DEFAULT '{}',
    status          SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Active, 3=Inactive
    version         INT         NOT NULL DEFAULT 1,
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_inspection_specs_doc_number ON inspection_specifications (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_inspection_specs_product ON inspection_specifications (product_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_inspection_specs_type ON inspection_specifications (inspection_type) WHERE deleted_at IS NULL;
CREATE INDEX idx_inspection_specs_status ON inspection_specifications (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 2. Inspection Results — 检验结果
-- ============================================================================

CREATE TABLE inspection_results (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(30) NOT NULL,
    spec_id         BIGINT      NOT NULL,
    source_type     SMALLINT    NOT NULL,           -- 1=ArrivalNotice, 2=WorkOrderRouting, 3=ShippingRequest, 4=OutsourcingOrder
    source_id       BIGINT      NOT NULL,
    inspection_type SMALLINT    NOT NULL,           -- 1=IQC, 2=IPQC, 3=FQC, 4=OQC
    batch_no        VARCHAR(80) NOT NULL DEFAULT '',
    sample_qty      DECIMAL(18,6) NOT NULL DEFAULT 0,
    qualified_qty   DECIMAL(18,6) NOT NULL DEFAULT 0,
    unqualified_qty DECIMAL(18,6) NOT NULL DEFAULT 0,
    result          SMALLINT    NOT NULL DEFAULT 1, -- 1=Pass, 2=Fail, 3=Conditional
    check_results   JSONB       NOT NULL DEFAULT '[]',
    inspector_id    BIGINT      NOT NULL DEFAULT 0,
    inspection_date DATE,
    status          SMALLINT    NOT NULL DEFAULT 1, -- 1=Pending, 2=Completed, 3=Dispositioned
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_inspection_results_doc_number ON inspection_results (doc_number) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX idx_inspection_results_idempotent
    ON inspection_results (source_type, source_id, inspection_type) WHERE deleted_at IS NULL;
CREATE INDEX idx_inspection_results_spec ON inspection_results (spec_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_inspection_results_source ON inspection_results (source_type, source_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_inspection_results_type ON inspection_results (inspection_type) WHERE deleted_at IS NULL;

-- ============================================================================
-- 3. MRB — 不良评审 (Material Review Board)
-- ============================================================================

CREATE TABLE mrbs (
    id                      BIGSERIAL   PRIMARY KEY,
    doc_number              VARCHAR(30) NOT NULL,
    inspection_result_id    BIGINT      NOT NULL,
    product_id              BIGINT      NOT NULL,
    defect_description      TEXT        NOT NULL DEFAULT '',
    disposition             SMALLINT    NOT NULL DEFAULT 1, -- 1=Scrap, 2=Return, 3=Degrade, 4=Rework
    responsible_party       SMALLINT    NOT NULL DEFAULT 1, -- 1=Internal, 2=Supplier, 3=Customer
    cost_impact             DECIMAL(20,4) NOT NULL DEFAULT 0,
    status                  SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=UnderReview, 3=Approved, 4=Completed
    remark                  TEXT        NOT NULL DEFAULT '',
    operator_id             BIGINT      NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at              TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_mrbs_doc_number ON mrbs (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_mrbs_inspection_result ON mrbs (inspection_result_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_mrbs_product ON mrbs (product_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_mrbs_status ON mrbs (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 4. RMA — 客诉追溯 (Return Merchandise Authorization)
-- ============================================================================

CREATE TABLE rmas (
    id                          BIGSERIAL   PRIMARY KEY,
    doc_number                  VARCHAR(30) NOT NULL,
    customer_id                 BIGINT      NOT NULL,
    sales_order_id              BIGINT,
    shipping_request_id         BIGINT,
    product_id                  BIGINT      NOT NULL,
    linked_inspection_result_id BIGINT,
    defect_description          TEXT        NOT NULL DEFAULT '',
    severity                    SMALLINT    NOT NULL DEFAULT 1, -- 1=Minor, 2=Major, 3=Critical
    root_cause                  TEXT,
    corrective_action           TEXT,
    status                      SMALLINT    NOT NULL DEFAULT 1, -- 1=Reported, 2=Investigating, 3=ActionTaken, 4=Closed
    remark                      TEXT        NOT NULL DEFAULT '',
    operator_id                 BIGINT      NOT NULL,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at                  TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_rmas_doc_number ON rmas (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_rmas_customer ON rmas (customer_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_rmas_product ON rmas (product_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_rmas_severity ON rmas (severity) WHERE deleted_at IS NULL;
CREATE INDEX idx_rmas_status ON rmas (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_rmas_inspection_result ON rmas (linked_inspection_result_id) WHERE deleted_at IS NULL;

-- ============================================================================
-- 5. State Machine Definitions — QMS 状态机配置
-- ============================================================================

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects) VALUES
-- InspectionSpecification
('InspectionSpecification', 'Draft', 'Active', NULL, NULL, '[]'),
('InspectionSpecification', 'Active', 'Inactive', NULL, NULL, '[]'),
-- InspectionResult
('InspectionResult', 'Pending', 'Completed', NULL, NULL, '[]'),
('InspectionResult', 'Completed', 'Dispositioned', NULL, NULL, '[]'),
-- MRB
('MRB', 'Draft', 'UnderReview', NULL, NULL, '[]'),
('MRB', 'UnderReview', 'Approved', NULL, NULL, '[]'),
('MRB', 'Approved', 'Completed', NULL, NULL, '[]'),
-- RMA
('RMA', 'Reported', 'Investigating', NULL, NULL, '[]'),
('RMA', 'Investigating', 'ActionTaken', NULL, NULL, '[]'),
('RMA', 'ActionTaken', 'Closed', NULL, NULL, '[]');

COMMIT;
