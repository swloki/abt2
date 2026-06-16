BEGIN;

-- ============================================================================
-- 1. 审批规则配置表
-- ============================================================================

CREATE TABLE purchase_approval_rules (
    id              BIGSERIAL      PRIMARY KEY,
    name            VARCHAR(64)    NOT NULL,
    min_amount      NUMERIC(20,4)  NOT NULL DEFAULT 0,
    max_amount      NUMERIC(20,4),
    approver_role   VARCHAR(64)    NOT NULL,
    approver_id     BIGINT,
    is_active       BOOLEAN        NOT NULL DEFAULT TRUE,
    sort_order      INTEGER        NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_par_active_amount ON purchase_approval_rules (is_active, min_amount)
    WHERE deleted_at IS NULL;

-- ============================================================================
-- 2. 状态机扩展：PendingApproval
-- ============================================================================

INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('PurchaseOrder', 'PendingApproval', '待审批', FALSE, FALSE)
ON CONFLICT DO NOTHING;

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PurchaseOrder', 'Draft',           'PendingApproval', NULL, 1.5),
    ('PurchaseOrder', 'PendingApproval', 'Confirmed',       NULL, 1.6),
    ('PurchaseOrder', 'PendingApproval', 'Draft',           NULL, 1.7),
    ('PurchaseOrder', 'PendingApproval', 'Cancelled',       NULL, 1.8)
ON CONFLICT DO NOTHING;

COMMIT;
