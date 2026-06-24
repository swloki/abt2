-- PickList 拣货单（Phase 3，Issue #93）
-- 独立拣货单：outbound（发货单）pick() 时生成，记录拣货数量/库位/拣货人，让拣货可追溯。
-- 1:1 outbound（一个发货单一个拣货单）。MVP：generate 时 picked_qty = requested_qty 自动满拣。
-- status: 1=Draft 2=Picked 3=Cancelled
-- 项目约定：无 FK 约束，应用层强制（outbound_id → shipping_requests.id, outbound_item_id → shipping_request_items.id）

CREATE TABLE pick_lists (
    id              BIGSERIAL      PRIMARY KEY,
    doc_number      VARCHAR(30)    NOT NULL UNIQUE,   -- PK-YYYY-MM-SEQ（DocumentType::PickList）
    outbound_id     BIGINT         NOT NULL,          -- → shipping_requests.id
    status          SMALLINT       NOT NULL DEFAULT 1,-- 1=Draft 2=Picked 3=Cancelled
    picker_id       BIGINT,                           -- 拣货人
    picked_at       TIMESTAMPTZ,                      -- 拣货完成时间
    remark          TEXT           NOT NULL DEFAULT '',
    operator_id     BIGINT         NOT NULL,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_pick_lists_outbound ON pick_lists(outbound_id);
CREATE INDEX idx_pick_lists_status ON pick_lists(status);

CREATE TABLE pick_list_items (
    id                  BIGSERIAL      PRIMARY KEY,
    pick_list_id        BIGINT         NOT NULL,          -- → pick_lists.id
    line_no             INT            NOT NULL,
    outbound_item_id    BIGINT         NOT NULL,          -- → shipping_request_items.id
    product_id          BIGINT         NOT NULL,
    warehouse_id        BIGINT         NOT NULL,
    bin_id              BIGINT,                           -- 拣货库位（可选）
    requested_qty       DECIMAL(18,6)  NOT NULL,          -- 请求数量（来自 outbound 明细）
    picked_qty          DECIMAL(18,6)  NOT NULL DEFAULT 0,-- 实拣数量（MVP=requested_qty 满拣）
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pick_list_items_pick_list ON pick_list_items(pick_list_id);
CREATE INDEX idx_pick_list_items_product ON pick_list_items(product_id);

-- PickListStatus 状态机：Draft → Picked / Cancelled
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PickListStatus', '', 'Draft', NULL, 1),
    ('PickListStatus', 'Draft', 'Picked', NULL, 2),
    ('PickListStatus', 'Draft', 'Cancelled', NULL, 3)
ON CONFLICT DO NOTHING;
