-- ============================================================================
-- ABT v2 Purchase Module (SRM) — 12 Tables
-- Database: abt_v2
-- No FK constraints (application-enforced, per project convention)
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Purchase Quotations — 采购报价主表
-- ============================================================================

CREATE TABLE purchase_quotations (
    id              BIGSERIAL      PRIMARY KEY,
    doc_number      VARCHAR(32)    NOT NULL,
    supplier_id     BIGINT         NOT NULL,
    quotation_date  DATE           NOT NULL,
    valid_from      DATE           NOT NULL,
    valid_until     DATE           NOT NULL,
    status          SMALLINT       NOT NULL DEFAULT 1, -- PurchaseQuotationStatus
    remark          TEXT           NOT NULL DEFAULT '',
    operator_id     BIGINT         NOT NULL,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_pq_doc_number ON purchase_quotations (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_pq_supplier ON purchase_quotations (supplier_id);
CREATE INDEX idx_pq_status ON purchase_quotations (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 2. Purchase Quotation Items — 采购报价明细
-- ============================================================================

CREATE TABLE purchase_quotation_items (
    id              BIGSERIAL      PRIMARY KEY,
    quotation_id    BIGINT         NOT NULL,
    product_id      BIGINT         NOT NULL,
    line_no         INTEGER        NOT NULL,
    unit_price      NUMERIC(18,6)  NOT NULL,
    min_order_qty   NUMERIC(18,6),
    lead_time_days  INTEGER,
    currency        VARCHAR(3)     NOT NULL DEFAULT 'CNY',
    is_preferred    BOOLEAN        NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_pqi_quotation ON purchase_quotation_items (quotation_id);
CREATE INDEX idx_pqi_product ON purchase_quotation_items (product_id);

-- ============================================================================
-- 3. Purchase Orders — 采购订单主表
-- ============================================================================

CREATE TABLE purchase_orders (
    id                      BIGSERIAL      PRIMARY KEY,
    doc_number              VARCHAR(32)    NOT NULL,
    supplier_id             BIGINT         NOT NULL,
    order_date              DATE           NOT NULL,
    expected_delivery_date  DATE,
    status                  SMALLINT       NOT NULL DEFAULT 1, -- PurchaseOrderStatus
    total_amount            NUMERIC(20,4)  NOT NULL DEFAULT 0,
    payment_terms           TEXT,
    delivery_address        TEXT,
    remark                  TEXT           NOT NULL DEFAULT '',
    operator_id             BIGINT         NOT NULL,
    created_at              TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at              TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_po_doc_number ON purchase_orders (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_po_supplier ON purchase_orders (supplier_id);
CREATE INDEX idx_po_status ON purchase_orders (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 4. Purchase Order Items — 采购订单明细
-- ============================================================================

CREATE TABLE purchase_order_items (
    id                      BIGSERIAL      PRIMARY KEY,
    order_id                BIGINT         NOT NULL,
    line_no                 INTEGER        NOT NULL,
    product_id              BIGINT         NOT NULL,
    description             TEXT           NOT NULL DEFAULT '',
    quantity                NUMERIC(18,6)  NOT NULL,
    unit_price              NUMERIC(18,6)  NOT NULL,
    amount                  NUMERIC(20,4)  NOT NULL,
    received_qty            NUMERIC(18,6)  NOT NULL DEFAULT 0,
    inspected_qty           NUMERIC(18,6)  NOT NULL DEFAULT 0,
    returned_qty            NUMERIC(18,6)  NOT NULL DEFAULT 0,
    quotation_item_id       BIGINT,
    expected_delivery_date  DATE
);

CREATE INDEX idx_poi_order ON purchase_order_items (order_id);
CREATE INDEX idx_poi_product ON purchase_order_items (product_id);

-- ============================================================================
-- 5. Purchase Returns — 采购退货主表
-- ============================================================================

CREATE TABLE purchase_returns (
    id              BIGSERIAL      PRIMARY KEY,
    doc_number      VARCHAR(32)    NOT NULL,
    order_id        BIGINT         NOT NULL,
    supplier_id     BIGINT         NOT NULL,
    return_date     DATE           NOT NULL,
    status          SMALLINT       NOT NULL DEFAULT 1, -- PurchaseReturnStatus
    return_reason   TEXT           NOT NULL,
    total_amount    NUMERIC(20,4)  NOT NULL DEFAULT 0,
    remark          TEXT           NOT NULL DEFAULT '',
    operator_id     BIGINT         NOT NULL,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_prt_doc_number ON purchase_returns (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_prt_order ON purchase_returns (order_id);
CREATE INDEX idx_prt_supplier ON purchase_returns (supplier_id);
CREATE INDEX idx_prt_status ON purchase_returns (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 6. Purchase Return Items — 采购退货明细
-- ============================================================================

CREATE TABLE purchase_return_items (
    id              BIGSERIAL      PRIMARY KEY,
    return_id       BIGINT         NOT NULL,
    order_item_id   BIGINT         NOT NULL,
    product_id      BIGINT         NOT NULL,
    returned_qty    NUMERIC(18,6)  NOT NULL,
    unit_price      NUMERIC(18,6)  NOT NULL,
    amount          NUMERIC(20,4)  NOT NULL
);

CREATE INDEX idx_pri_return ON purchase_return_items (return_id);

-- ============================================================================
-- 7. Purchase Reconciliations — 采购对账单主表
-- ============================================================================

CREATE TABLE purchase_reconciliations (
    id                BIGSERIAL      PRIMARY KEY,
    doc_number        VARCHAR(32)    NOT NULL,
    supplier_id       BIGINT         NOT NULL,
    period            VARCHAR(7)     NOT NULL,  -- "2026-05"
    status            SMALLINT       NOT NULL DEFAULT 1, -- PurchaseReconStatus
    total_amount      NUMERIC(20,4)  NOT NULL DEFAULT 0,
    confirmed_amount  NUMERIC(20,4)  NOT NULL DEFAULT 0,
    difference        NUMERIC(20,4)  NOT NULL DEFAULT 0,
    remark            TEXT           NOT NULL DEFAULT '',
    operator_id       BIGINT         NOT NULL,
    created_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at        TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_prc_doc_number ON purchase_reconciliations (doc_number) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX idx_prc_supplier_period ON purchase_reconciliations (supplier_id, period) WHERE deleted_at IS NULL;
CREATE INDEX idx_prc_status ON purchase_reconciliations (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 8. Purchase Reconciliation Items — 对账明细
-- ============================================================================

CREATE TABLE purchase_recon_items (
    id                  BIGSERIAL      PRIMARY KEY,
    reconciliation_id   BIGINT         NOT NULL,
    order_id            BIGINT         NOT NULL,
    order_item_id       BIGINT         NOT NULL,
    received_qty        NUMERIC(18,6)  NOT NULL,
    returned_qty        NUMERIC(18,6)  NOT NULL DEFAULT 0,
    returned_amount     NUMERIC(20,4)  NOT NULL DEFAULT 0,
    unit_price          NUMERIC(18,6)  NOT NULL,
    amount              NUMERIC(20,4)  NOT NULL,
    confirmed           BOOLEAN        NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_prci_reconciliation ON purchase_recon_items (reconciliation_id);

-- ============================================================================
-- 9. Payment Requests — 付款申请主表
-- ============================================================================

CREATE TABLE payment_requests (
    id                  BIGSERIAL      PRIMARY KEY,
    doc_number          VARCHAR(32)    NOT NULL,
    supplier_id         BIGINT         NOT NULL,
    reconciliation_id   BIGINT,
    payment_date        DATE           NOT NULL,
    amount              NUMERIC(20,4)  NOT NULL,
    status              SMALLINT       NOT NULL DEFAULT 1, -- PaymentStatus
    payment_method      SMALLINT       NOT NULL,           -- PaymentMethod
    bank_account_id     BIGINT,
    invoice_number      VARCHAR(64),
    invoice_amount      NUMERIC(20,4),
    remark              TEXT           NOT NULL DEFAULT '',
    operator_id         BIGINT         NOT NULL,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_pay_doc_number ON payment_requests (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_pay_supplier ON payment_requests (supplier_id);
CREATE INDEX idx_pay_status ON payment_requests (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 10. Miscellaneous Requests — 零星请购主表
-- ============================================================================

CREATE TABLE miscellaneous_requests (
    id              BIGSERIAL      PRIMARY KEY,
    doc_number      VARCHAR(32)    NOT NULL,
    department_id   BIGINT         NOT NULL,
    request_date    DATE           NOT NULL,
    status          SMALLINT       NOT NULL DEFAULT 1, -- MiscRequestStatus
    total_amount    NUMERIC(20,4)  NOT NULL DEFAULT 0,
    purpose         TEXT           NOT NULL,
    remark          TEXT           NOT NULL DEFAULT '',
    operator_id     BIGINT         NOT NULL,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_misc_doc_number ON miscellaneous_requests (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_misc_department ON miscellaneous_requests (department_id);
CREATE INDEX idx_misc_status ON miscellaneous_requests (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 11. Miscellaneous Request Items — 零星请购明细
-- ============================================================================

CREATE TABLE misc_request_items (
    id               BIGSERIAL      PRIMARY KEY,
    request_id       BIGINT         NOT NULL,
    line_no          INTEGER        NOT NULL,
    item_name        TEXT           NOT NULL,
    specification    TEXT,
    quantity         NUMERIC(18,6)  NOT NULL,
    unit             VARCHAR(16)    NOT NULL,
    estimated_price  NUMERIC(18,6),
    remark           TEXT
);

CREATE INDEX idx_mri_request ON misc_request_items (request_id);

-- ============================================================================
-- 12. State Machine — Purchase State Definitions
-- ============================================================================

-- PurchaseQuotation: Draft(1) → Active(2) → Expired(3), Draft → Cancelled(4)
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PurchaseQuotation', 'Draft',     '草稿',   TRUE,  FALSE),
    ('PurchaseQuotation', 'Active',    '生效',   FALSE, FALSE),
    ('PurchaseQuotation', 'Expired',   '过期',   FALSE, TRUE),
    ('PurchaseQuotation', 'Cancelled', '已取消', FALSE, TRUE);

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PurchaseQuotation', 'Draft', 'Active',    NULL, 1),
    ('PurchaseQuotation', 'Draft', 'Cancelled', NULL, 2),
    ('PurchaseQuotation', 'Active', 'Expired',  NULL, 3);

-- PurchaseOrder: Draft(1) → Confirmed(2) → PartiallyReceived(3) → Received(4) → Closed(5), Draft → Cancelled(6)
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PurchaseOrder', 'Draft',              '草稿',     TRUE,  FALSE),
    ('PurchaseOrder', 'Confirmed',          '已确认',   FALSE, FALSE),
    ('PurchaseOrder', 'PartiallyReceived',  '部分收货', FALSE, FALSE),
    ('PurchaseOrder', 'Received',           '已收货',   FALSE, FALSE),
    ('PurchaseOrder', 'Closed',             '已关闭',   FALSE, TRUE),
    ('PurchaseOrder', 'Cancelled',          '已取消',   FALSE, TRUE);

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PurchaseOrder', 'Draft',             'Confirmed',         NULL, 1),
    ('PurchaseOrder', 'Draft',             'Cancelled',         NULL, 2),
    ('PurchaseOrder', 'Confirmed',         'PartiallyReceived', NULL, 3),
    ('PurchaseOrder', 'Confirmed',         'Received',          NULL, 4),
    ('PurchaseOrder', 'PartiallyReceived', 'Received',          NULL, 5),
    ('PurchaseOrder', 'Received',          'Closed',            NULL, 6);

-- PurchaseReturn: Draft(1) → Confirmed(2) → Shipped(3) → Settled(4), Draft → Cancelled(5)
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PurchaseReturn', 'Draft',     '草稿',   TRUE,  FALSE),
    ('PurchaseReturn', 'Confirmed', '已确认', FALSE, FALSE),
    ('PurchaseReturn', 'Shipped',   '已发货', FALSE, FALSE),
    ('PurchaseReturn', 'Settled',   '已结算', FALSE, TRUE),
    ('PurchaseReturn', 'Cancelled', '已取消', FALSE, TRUE);

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PurchaseReturn', 'Draft',     'Confirmed', NULL, 1),
    ('PurchaseReturn', 'Draft',     'Cancelled', NULL, 2),
    ('PurchaseReturn', 'Confirmed', 'Shipped',   NULL, 3),
    ('PurchaseReturn', 'Shipped',   'Settled',   NULL, 4);

-- PurchaseReconciliation: Draft(1) → Confirmed(2) → Settled(3)
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PurchaseReconciliation', 'Draft',     '草稿',   TRUE,  FALSE),
    ('PurchaseReconciliation', 'Confirmed', '已确认', FALSE, FALSE),
    ('PurchaseReconciliation', 'Settled',   '已结算', FALSE, TRUE);

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PurchaseReconciliation', 'Draft',     'Confirmed', NULL, 1),
    ('PurchaseReconciliation', 'Confirmed', 'Settled',   NULL, 2);

-- PaymentRequest: Draft(1) → Approved(2) → Paid(3), Draft → Cancelled(4)
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PaymentRequest', 'Draft',     '草稿',   TRUE,  FALSE),
    ('PaymentRequest', 'Approved',  '已审批', FALSE, FALSE),
    ('PaymentRequest', 'Paid',      '已付款', FALSE, TRUE),
    ('PaymentRequest', 'Cancelled', '已取消', FALSE, TRUE);

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PaymentRequest', 'Draft',    'Approved',  NULL, 1),
    ('PaymentRequest', 'Draft',    'Cancelled', NULL, 2),
    ('PaymentRequest', 'Approved', 'Paid',      NULL, 3);

-- MiscellaneousRequest: Draft(1) → Approved(2) → Purchasing(3) → Received(4) → Closed(5), Draft → Cancelled(6)
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('MiscellaneousRequest', 'Draft',      '草稿',   TRUE,  FALSE),
    ('MiscellaneousRequest', 'Approved',   '已审批', FALSE, FALSE),
    ('MiscellaneousRequest', 'Purchasing', '采购中', FALSE, FALSE),
    ('MiscellaneousRequest', 'Received',   '已收货', FALSE, FALSE),
    ('MiscellaneousRequest', 'Closed',     '已关闭', FALSE, TRUE),
    ('MiscellaneousRequest', 'Cancelled',  '已取消', FALSE, TRUE);

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('MiscellaneousRequest', 'Draft',      'Approved',   NULL, 1),
    ('MiscellaneousRequest', 'Draft',      'Cancelled',  NULL, 2),
    ('MiscellaneousRequest', 'Approved',   'Purchasing', NULL, 3),
    ('MiscellaneousRequest', 'Purchasing', 'Received',   NULL, 4),
    ('MiscellaneousRequest', 'Received',   'Closed',     NULL, 5);

COMMIT;
