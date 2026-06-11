-- ============================================================================
-- WMS Module — Warehouse Management System
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Warehouses — 仓库
-- ============================================================================

CREATE TABLE warehouses (
    id              BIGSERIAL   PRIMARY KEY,
    code            VARCHAR(50) NOT NULL,
    name            VARCHAR(200) NOT NULL,
    warehouse_type  SMALLINT    NOT NULL,           -- 1=RawMaterial, 2=FinishedGoods, 3=SemiFinished, 4=Consumable, 5=VirtualOutsource
    status          SMALLINT    NOT NULL DEFAULT 1, -- 1=Active, 2=Inactive
    address         TEXT,
    manager_id      BIGINT,
    is_virtual      BOOLEAN     NOT NULL DEFAULT false,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    UNIQUE (code)
);

CREATE INDEX idx_warehouses_type ON warehouses (warehouse_type) WHERE deleted_at IS NULL;
CREATE INDEX idx_warehouses_active ON warehouses (id) WHERE deleted_at IS NULL;

-- ============================================================================
-- 2. Zones — 库区
-- ============================================================================

CREATE TABLE zones (
    id              BIGSERIAL   PRIMARY KEY,
    warehouse_id    BIGINT      NOT NULL REFERENCES warehouses(id),
    code            VARCHAR(50) NOT NULL,
    name            VARCHAR(200) NOT NULL,
    zone_type       SMALLINT    NOT NULL,           -- 1=Receiving, 2=Storage, 3=Picking, 4=Packing, 5=Inspection, 6=Returns
    sort_order      INT         NOT NULL DEFAULT 0,
    remark          TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    UNIQUE (warehouse_id, code)
);

-- ============================================================================
-- 3. Bins — 库位
-- ============================================================================

CREATE TABLE bins (
    id                      BIGSERIAL   PRIMARY KEY,
    zone_id                 BIGINT      NOT NULL REFERENCES zones(id),
    code                    VARCHAR(50) NOT NULL,
    name                    VARCHAR(200) NOT NULL,
    row_no                  VARCHAR(20),
    column_no               VARCHAR(20),
    layer_no                VARCHAR(20),
    capacity_limit          DECIMAL(18,6),
    allowed_product_types   TEXT[],
    temperature_req         VARCHAR(50),
    status                  SMALLINT    NOT NULL DEFAULT 1, -- 1=Empty, 2=Occupied, 3=Locked, 4=Disabled
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at              TIMESTAMPTZ,

    UNIQUE (zone_id, code)
);

-- ============================================================================
-- 4. Putaway Strategies — 上架策略
-- ============================================================================

CREATE TABLE putaway_strategies (
    id                      BIGSERIAL   PRIMARY KEY,
    name                    VARCHAR(200) NOT NULL,
    strategy_type           SMALLINT    NOT NULL,           -- 1=SameMerge, 2=Nearest, 3=FixedBin, 4=EmptyFirst
    warehouse_id            BIGINT      REFERENCES warehouses(id),
    product_category_id     BIGINT,
    priority                INT         NOT NULL DEFAULT 0,
    is_active               BOOLEAN     NOT NULL DEFAULT true
);

-- ============================================================================
-- 5. Pick Strategies — 拣货策略
-- ============================================================================

CREATE TABLE pick_strategies (
    id              BIGSERIAL   PRIMARY KEY,
    name            VARCHAR(200) NOT NULL,
    strategy_type   SMALLINT    NOT NULL,           -- 1=FIFO, 2=FEFO, 3=ShortestPath, 4=FullPallet
    warehouse_id    BIGINT      REFERENCES warehouses(id),
    priority        INT         NOT NULL DEFAULT 0,
    is_active       BOOLEAN     NOT NULL DEFAULT true
);

-- ============================================================================
-- 6. Stock Ledger — 库存台账
-- ============================================================================

CREATE TABLE stock_ledger (
    id              BIGSERIAL   PRIMARY KEY,
    product_id      BIGINT      NOT NULL,
    warehouse_id    BIGINT      NOT NULL,
    zone_id         BIGINT      NOT NULL,
    bin_id          BIGINT      NOT NULL,
    batch_no        VARCHAR(50),
    quantity        DECIMAL(18,6) NOT NULL DEFAULT 0,
    reserved_qty    DECIMAL(18,6) NOT NULL DEFAULT 0,
    available_qty   DECIMAL(18,6) NOT NULL DEFAULT 0,
    unit_cost       DECIMAL(18,6),
    received_date   DATE,
    expiry_date     DATE,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_stock_ledger_unique ON stock_ledger (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, ''));
CREATE INDEX idx_stock_product ON stock_ledger (product_id);
CREATE INDEX idx_stock_warehouse ON stock_ledger (warehouse_id);

-- ============================================================================
-- 7. Arrival Notices — 来料通知
-- ============================================================================

CREATE TABLE arrival_notices (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    purchase_order_id   BIGINT,
    supplier_id         BIGINT      NOT NULL,
    arrival_date        DATE        NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Received, 3=Inspecting, 4=Accepted, 5=PartiallyAccepted, 6=Rejected, 7=Cancelled
    warehouse_id        BIGINT      NOT NULL REFERENCES warehouses(id),
    zone_id             BIGINT      REFERENCES zones(id),
    delivery_note       TEXT,
    remark              TEXT        NOT NULL DEFAULT '',
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ,

    UNIQUE (doc_number)
);

CREATE INDEX idx_arrival_status ON arrival_notices (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_arrival_supplier ON arrival_notices (supplier_id) WHERE deleted_at IS NULL;

-- ============================================================================
-- 8. Arrival Notice Items — 来料通知明细
-- ============================================================================

CREATE TABLE arrival_notice_items (
    id              BIGSERIAL   PRIMARY KEY,
    notice_id       BIGINT      NOT NULL REFERENCES arrival_notices(id),
    order_item_id   BIGINT,
    product_id      BIGINT      NOT NULL,
    declared_qty    DECIMAL(18,6) NOT NULL,
    received_qty    DECIMAL(18,6) NOT NULL DEFAULT 0,
    accepted_qty    DECIMAL(18,6) NOT NULL DEFAULT 0,
    batch_no        VARCHAR(50)
);

CREATE INDEX idx_ani_notice ON arrival_notice_items (notice_id);

-- ============================================================================
-- 9. Inventory Transactions — 库存事务（Append-only）
-- ============================================================================

CREATE TABLE inventory_transactions (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50),
    transaction_type    SMALLINT    NOT NULL,       -- 1=PurchaseReceipt, 2=ProductionReceipt, 3=SalesShipment, 4=MaterialIssue, 5=MaterialReturn, 6=Backflush, 7=Transfer, 8=FormConversion, 9=Adjustment, 10=Lock, 11=Unlock, 12=Scrap
    product_id          BIGINT      NOT NULL,
    warehouse_id        BIGINT      NOT NULL,
    zone_id             BIGINT,
    bin_id              BIGINT,
    batch_no            VARCHAR(50),
    quantity            DECIMAL(18,6) NOT NULL,
    unit_cost           DECIMAL(18,6),
    source_type         VARCHAR(50) NOT NULL,       -- DocumentType string reference
    source_id           BIGINT      NOT NULL,
    remark              TEXT,
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_txn_product ON inventory_transactions (product_id);
CREATE INDEX idx_txn_source ON inventory_transactions (source_type, source_id);
CREATE INDEX idx_txn_type ON inventory_transactions (transaction_type);
CREATE INDEX idx_txn_created ON inventory_transactions (created_at);

-- ============================================================================
-- 10. Material Requisitions — 领料单
-- ============================================================================

CREATE TABLE material_requisitions (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    work_order_id       BIGINT      NOT NULL,
    requisition_date    DATE        NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Confirmed, 3=Issued, 4=Cancelled
    warehouse_id        BIGINT      NOT NULL REFERENCES warehouses(id),
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ,

    UNIQUE (doc_number)
);

CREATE INDEX idx_req_status ON material_requisitions (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_req_wo ON material_requisitions (work_order_id) WHERE deleted_at IS NULL;

-- ============================================================================
-- 11. Material Requisition Items — 领料单明细
-- ============================================================================

CREATE TABLE material_requisition_items (
    id                  BIGSERIAL   PRIMARY KEY,
    requisition_id      BIGINT      NOT NULL REFERENCES material_requisitions(id),
    product_id          BIGINT      NOT NULL,
    requested_qty       DECIMAL(18,6) NOT NULL,
    issued_qty          DECIMAL(18,6) NOT NULL DEFAULT 0,
    variance_qty        DECIMAL(18,6) NOT NULL DEFAULT 0,
    bin_id              BIGINT
);

CREATE INDEX idx_mri_requisition ON material_requisition_items (requisition_id);

-- ============================================================================
-- 12. Backflush Records — 倒冲记录
-- ============================================================================

CREATE TABLE backflush_records (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    work_order_id       BIGINT      NOT NULL,
    product_id          BIGINT      NOT NULL,
    completed_qty       DECIMAL(18,6) NOT NULL,
    backflush_date      DATE        NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Executed, 3=Adjusted
    variance_threshold  DECIMAL(18,6) NOT NULL DEFAULT 0.05,
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number)
);

-- ============================================================================
-- 13. Backflush Items — 倒冲明细
-- ============================================================================

CREATE TABLE backflush_items (
    id                  BIGSERIAL   PRIMARY KEY,
    record_id           BIGINT      NOT NULL REFERENCES backflush_records(id),
    component_id        BIGINT      NOT NULL,
    theoretical_qty     DECIMAL(18,6) NOT NULL,
    actual_qty          DECIMAL(18,6) NOT NULL,
    variance_qty        DECIMAL(18,6) NOT NULL,
    variance_rate       DECIMAL(18,6) NOT NULL,
    is_over_threshold   BOOLEAN     NOT NULL DEFAULT false
);

CREATE INDEX idx_bi_record ON backflush_items (record_id);

-- ============================================================================
-- 14. Cycle Counts — 盘点单
-- ============================================================================

CREATE TABLE cycle_counts (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    warehouse_id        BIGINT      NOT NULL REFERENCES warehouses(id),
    zone_id             BIGINT      REFERENCES zones(id),
    count_date          DATE        NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Counting, 3=Completed, 4=Adjusted, 5=Cancelled
    is_blind            BOOLEAN     NOT NULL DEFAULT false,
    remark              TEXT,
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number)
);

-- ============================================================================
-- 15. Cycle Count Items — 盘点明细
-- ============================================================================

CREATE TABLE cycle_count_items (
    id                  BIGSERIAL   PRIMARY KEY,
    count_id            BIGINT      NOT NULL REFERENCES cycle_counts(id),
    bin_id              BIGINT      NOT NULL,
    product_id          BIGINT      NOT NULL,
    batch_no            VARCHAR(50),
    system_qty          DECIMAL(18,6) NOT NULL,
    counted_qty         DECIMAL(18,6) NOT NULL DEFAULT 0,
    variance_qty        DECIMAL(18,6) NOT NULL DEFAULT 0,
    variance_reason     TEXT,
    is_adjusted         BOOLEAN     NOT NULL DEFAULT false
);

CREATE INDEX idx_cci_count ON cycle_count_items (count_id);

-- ============================================================================
-- 16. Inventory Transfers — 调拨单
-- ============================================================================

CREATE TABLE inventory_transfers (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    from_warehouse_id   BIGINT      NOT NULL REFERENCES warehouses(id),
    from_zone_id        BIGINT      REFERENCES zones(id),
    from_bin_id         BIGINT      REFERENCES bins(id),
    to_warehouse_id     BIGINT      NOT NULL REFERENCES warehouses(id),
    to_zone_id          BIGINT      REFERENCES zones(id),
    to_bin_id           BIGINT      REFERENCES bins(id),
    transfer_date       DATE        NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=InTransit, 3=Completed, 4=Cancelled
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number)
);

-- ============================================================================
-- 17. Transfer Items — 调拨明细
-- ============================================================================

CREATE TABLE transfer_items (
    id                  BIGSERIAL   PRIMARY KEY,
    transfer_id         BIGINT      NOT NULL REFERENCES inventory_transfers(id),
    product_id          BIGINT      NOT NULL,
    quantity            DECIMAL(18,6) NOT NULL,
    batch_no            VARCHAR(50)
);

CREATE INDEX idx_ti_transfer ON transfer_items (transfer_id);

-- ============================================================================
-- 18. Form Conversions — 形态转换
-- ============================================================================

CREATE TABLE form_conversions (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    warehouse_id        BIGINT      NOT NULL REFERENCES warehouses(id),
    conversion_date     DATE        NOT NULL,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Draft, 2=Completed, 3=Cancelled
    remark              TEXT        NOT NULL DEFAULT '',
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number)
);

-- ============================================================================
-- 19. Conversion Items — 形态转换明细
-- ============================================================================

CREATE TABLE conversion_items (
    id                  BIGSERIAL   PRIMARY KEY,
    conversion_id       BIGINT      NOT NULL REFERENCES form_conversions(id),
    direction           SMALLINT    NOT NULL,           -- 1=Consume, 2=Produce
    product_id          BIGINT      NOT NULL,
    quantity            DECIMAL(18,6) NOT NULL,
    unit_cost           DECIMAL(18,6) NOT NULL,
    batch_no            VARCHAR(50)
);

CREATE INDEX idx_ci_conversion ON conversion_items (conversion_id);

-- ============================================================================
-- 20. Inventory Locks — 锁库
-- ============================================================================

CREATE TABLE inventory_locks (
    id                  BIGSERIAL   PRIMARY KEY,
    doc_number          VARCHAR(50) NOT NULL,
    product_id          BIGINT      NOT NULL,
    warehouse_id        BIGINT      NOT NULL REFERENCES warehouses(id),
    locked_qty          DECIMAL(18,6) NOT NULL,
    lock_reason         TEXT        NOT NULL,
    customer_id         BIGINT,
    status              SMALLINT    NOT NULL DEFAULT 1, -- 1=Active, 2=Released, 3=Cancelled
    operator_id         BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (doc_number)
);

-- ============================================================================
-- 21. State Machine Definitions — WMS 状态机配置
-- ============================================================================

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects) VALUES
-- ArrivalNotice
('ArrivalNotice', 'Draft', 'Received', NULL, NULL, '[]'),
('ArrivalNotice', 'Received', 'Inspecting', NULL, NULL, '[]'),
('ArrivalNotice', 'Inspecting', 'Accepted', NULL, NULL, '[]'),
('ArrivalNotice', 'Inspecting', 'PartiallyAccepted', NULL, NULL, '[]'),
('ArrivalNotice', 'Inspecting', 'Rejected', NULL, NULL, '[]'),
('ArrivalNotice', 'Draft', 'Cancelled', NULL, NULL, '[]'),
-- MaterialRequisition
('MaterialRequisition', 'Draft', 'Confirmed', NULL, NULL, '[]'),
('MaterialRequisition', 'Confirmed', 'Issued', NULL, NULL, '[]'),
('MaterialRequisition', 'Draft', 'Cancelled', NULL, NULL, '[]'),
('MaterialRequisition', 'Confirmed', 'Cancelled', NULL, NULL, '[]'),
-- BackflushRecord
('BackflushRecord', 'Draft', 'Executed', NULL, NULL, '[]'),
('BackflushRecord', 'Executed', 'Adjusted', NULL, NULL, '[]'),
-- CycleCount
('CycleCount', 'Draft', 'Counting', NULL, NULL, '[]'),
('CycleCount', 'Counting', 'Completed', NULL, NULL, '[]'),
('CycleCount', 'Completed', 'Adjusted', NULL, NULL, '[]'),
('CycleCount', 'Draft', 'Cancelled', NULL, NULL, '[]'),
('CycleCount', 'Counting', 'Cancelled', NULL, NULL, '[]'),
-- InventoryTransfer
('InventoryTransfer', 'Draft', 'InTransit', NULL, NULL, '[]'),
('InventoryTransfer', 'InTransit', 'Completed', NULL, NULL, '[]'),
('InventoryTransfer', 'Draft', 'Cancelled', NULL, NULL, '[]'),
-- FormConversion
('FormConversion', 'Draft', 'Completed', NULL, NULL, '[]'),
('FormConversion', 'Draft', 'Cancelled', NULL, NULL, '[]'),
-- InventoryLock
('InventoryLock', 'Active', 'Released', NULL, NULL, '[]'),
('InventoryLock', 'Active', 'Cancelled', NULL, NULL, '[]');

-- Partial unique indexes (soft-delete safe uniqueness)
CREATE UNIQUE INDEX idx_zones_unique_active ON zones (warehouse_id, code) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX idx_bins_unique_active ON bins (zone_id, code) WHERE deleted_at IS NULL;

COMMIT;
