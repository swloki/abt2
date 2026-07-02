-- stock_pickings 统一库存作业单据（Issue #146）
-- 把收货/发货/领料/调拨 4 类作业单据统一为一张表，按 picking_type 区分业务，统一状态机。
-- 参照 Odoo stock.picking 统一表派。底层库存流水仍走 inventory_transactions（本表 done 时写流水）。
-- 盘点 cycle_counts 保持独立，不纳入本表。
--
-- picking_type SMALLINT: 1=IncomingPurchase 采购收货 / 2=IncomingWorkOrder 生产入库 /
--   3=OutgoingSales 销售发货 / 4=InternalTransfer 库存调拨 / 5=InternalIssue 生产领料
-- status SMALLINT: 1=Draft / 2=Confirmed / 3=Done / 4=Cancelled
-- 项目约定：无 FK 约束，应用层强制

CREATE TABLE stock_pickings (
    id                BIGSERIAL      PRIMARY KEY,
    doc_number        VARCHAR(30)    NOT NULL UNIQUE,
    picking_type      SMALLINT       NOT NULL,             -- PickingType
    status            SMALLINT       NOT NULL DEFAULT 1,   -- PickingStatus
    source_type       VARCHAR(30)    NOT NULL DEFAULT 'none', -- purchase_order/work_order/sales_order/none
    source_id         BIGINT,                              -- 来源单据 id
    partner_id        BIGINT,                              -- 客户/供应商
    from_warehouse_id BIGINT,                              -- 源库位（发货/调拨/领料出库侧）
    from_zone_id      BIGINT,
    from_bin_id       BIGINT,
    to_warehouse_id   BIGINT,                              -- 目标库位（收货/调拨入库侧）
    to_zone_id        BIGINT,
    to_bin_id         BIGINT,
    operator_id       BIGINT         NOT NULL,
    scheduled_date    DATE,                                -- 计划日期（驱动紧急度）
    done_at           TIMESTAMPTZ,                         -- 完成时间
    pick_list_id      BIGINT,                              -- 关联拣货单（发货拣货子流程）
    work_order_id     BIGINT,                              -- 关联工单（领料/生产入库）
    remark            TEXT           NOT NULL DEFAULT '',
    created_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at        TIMESTAMPTZ
);

CREATE INDEX idx_stock_pickings_type       ON stock_pickings(picking_type);
CREATE INDEX idx_stock_pickings_status     ON stock_pickings(status);
CREATE INDEX idx_stock_pickings_source     ON stock_pickings(source_type, source_id) WHERE source_id IS NOT NULL;
CREATE INDEX idx_stock_pickings_work_order ON stock_pickings(work_order_id) WHERE work_order_id IS NOT NULL;
CREATE INDEX idx_stock_pickings_deleted_at ON stock_pickings(deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE stock_picking_items (
    id              BIGSERIAL      PRIMARY KEY,
    picking_id      BIGINT         NOT NULL,               -- → stock_pickings.id
    product_id      BIGINT         NOT NULL,
    batch_no        VARCHAR(50),                           -- 批次
    qty_requested   DECIMAL(18,6)  NOT NULL,               -- 申请/需求量
    qty_done        DECIMAL(18,6)  NOT NULL DEFAULT 0,     -- 实际量（行级部分完成：qty_done < qty_requested）
    from_bin_id     BIGINT,                                -- 行级源库位（拣货/出库）
    to_bin_id       BIGINT,                                -- 行级目标库位（上架/入库）
    operation_id    BIGINT,                                -- 工序（领料用）
    source_item_id  BIGINT,                                -- 来源单据明细行（PO/SO 行）
    remark          TEXT           NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_stock_picking_items_picking ON stock_picking_items(picking_id);
CREATE INDEX idx_stock_picking_items_product ON stock_picking_items(product_id);

-- PickingStatus 状态机：Draft → Confirmed → Done；Draft/Confirmed → Cancelled
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PickingStatus', '', 'Draft', NULL, 1),
    ('PickingStatus', 'Draft', 'Confirmed', NULL, 2),
    ('PickingStatus', 'Confirmed', 'Done', NULL, 3),
    ('PickingStatus', 'Draft', 'Cancelled', NULL, 4),
    ('PickingStatus', 'Confirmed', 'Cancelled', NULL, 5)
ON CONFLICT DO NOTHING;
