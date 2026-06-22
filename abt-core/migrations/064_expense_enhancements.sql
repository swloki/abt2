-- ============================================================================
-- ABT v2 — Expense Reimbursement Enhancements (Issue #63)
-- 1. Add fields to expense_reimbursements (sheet_count, has_invoice, payment info, supervisor)
-- 2. Add fields to expense_reimbursement_items (occurrence_date, has_invoice)
-- 3. Create expense_attachments table
-- 4. Add leader_id to departments
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. expense_reimbursements — 新增字段
-- ============================================================================

ALTER TABLE expense_reimbursements
  ADD COLUMN IF NOT EXISTS sheet_count    INTEGER NOT NULL DEFAULT 1,
  ADD COLUMN IF NOT EXISTS has_invoice    BOOLEAN NOT NULL DEFAULT TRUE,
  ADD COLUMN IF NOT EXISTS payment_remark TEXT,
  ADD COLUMN IF NOT EXISTS payment_bank   VARCHAR(128),
  ADD COLUMN IF NOT EXISTS payment_date   DATE,
  ADD COLUMN IF NOT EXISTS supervisor_id  BIGINT;

-- ============================================================================
-- 2. expense_reimbursement_items — 新增字段
-- ============================================================================

ALTER TABLE expense_reimbursement_items
  ADD COLUMN IF NOT EXISTS occurrence_date DATE,
  ADD COLUMN IF NOT EXISTS has_invoice     BOOLEAN NOT NULL DEFAULT TRUE;

-- ============================================================================
-- 3. expense_attachments — 报销凭证附件表
-- ============================================================================

CREATE TABLE IF NOT EXISTS expense_attachments (
    id              BIGSERIAL PRIMARY KEY,
    expense_id      BIGINT         NOT NULL,
    file_name       VARCHAR(256)   NOT NULL,
    file_path       TEXT           NOT NULL,
    mime_type       VARCHAR(128)   NOT NULL DEFAULT 'image/jpeg',
    file_size       INTEGER        NOT NULL DEFAULT 0,
    sort_order      INTEGER        NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ea_expense ON expense_attachments (expense_id);

-- ============================================================================
-- 4. departments — 新增部门负责人字段（直属上级）
-- ============================================================================

ALTER TABLE departments
  ADD COLUMN IF NOT EXISTS leader_id BIGINT;

-- ============================================================================
-- 5. ExpenseStatus 状态机配置 — 新增状态定义和转换规则
-- ============================================================================

-- 状态定义（插入新状态，已存在的跳过）
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final)
SELECT 'ExpenseStatus', 'SupervisorApproved', '直属上级已批', FALSE, FALSE
WHERE NOT EXISTS (
    SELECT 1 FROM state_definitions WHERE entity_type = 'ExpenseStatus' AND state_name = 'SupervisorApproved'
);

INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final)
SELECT 'ExpenseStatus', 'FinanceApproved', '财务已审', FALSE, FALSE
WHERE NOT EXISTS (
    SELECT 1 FROM state_definitions WHERE entity_type = 'ExpenseStatus' AND state_name = 'FinanceApproved'
);

-- 更新 Approved 为非终态（审批链中有后续 Paid 状态）
UPDATE state_definitions SET is_final = FALSE
WHERE entity_type = 'ExpenseStatus' AND state_name = 'Approved';

-- 转换规则（插入新的，已存在的跳过）
-- Draft → Submitted 改为 Submitted → SupervisorApproved，保留原链路

-- Submitted → SupervisorApproved（直属上级审批）
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
SELECT 'ExpenseStatus', 'Submitted', 'SupervisorApproved', NULL, NULL, '[]'::jsonb, 21
WHERE NOT EXISTS (
    SELECT 1 FROM state_transition_defs WHERE entity_type = 'ExpenseStatus' AND from_state = 'Submitted' AND to_state = 'SupervisorApproved'
);

-- SupervisorApproved → FinanceApproved（财务审批）
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
SELECT 'ExpenseStatus', 'SupervisorApproved', 'FinanceApproved', NULL, NULL, '[]'::jsonb, 22
WHERE NOT EXISTS (
    SELECT 1 FROM state_transition_defs WHERE entity_type = 'ExpenseStatus' AND from_state = 'SupervisorApproved' AND to_state = 'FinanceApproved'
);

-- FinanceApproved → Approved（总经理审批）
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
SELECT 'ExpenseStatus', 'FinanceApproved', 'Approved', NULL, NULL, '[]'::jsonb, 23
WHERE NOT EXISTS (
    SELECT 1 FROM state_transition_defs WHERE entity_type = 'ExpenseStatus' AND from_state = 'FinanceApproved' AND to_state = 'Approved'
);

-- 注意：已有的 Submitted → Approved 转换规则可能需要保留或删除
-- 删除旧的直接 Submitted → Approved 跳过中间审批的转换
DELETE FROM state_transition_defs
WHERE entity_type = 'ExpenseStatus' AND from_state = 'Submitted' AND to_state = 'Approved';

-- 新增取消路径
-- Draft → Cancelled
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
SELECT 'ExpenseStatus', 'Draft', 'Cancelled', NULL, NULL, '[]'::jsonb, 30
WHERE NOT EXISTS (
    SELECT 1 FROM state_transition_defs WHERE entity_type = 'ExpenseStatus' AND from_state = 'Draft' AND to_state = 'Cancelled'
);

-- Submitted → Cancelled
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
SELECT 'ExpenseStatus', 'Submitted', 'Cancelled', NULL, NULL, '[]'::jsonb, 31
WHERE NOT EXISTS (
    SELECT 1 FROM state_transition_defs WHERE entity_type = 'ExpenseStatus' AND from_state = 'Submitted' AND to_state = 'Cancelled'
);

-- SupervisorApproved → Cancelled
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
SELECT 'ExpenseStatus', 'SupervisorApproved', 'Cancelled', NULL, NULL, '[]'::jsonb, 32
WHERE NOT EXISTS (
    SELECT 1 FROM state_transition_defs WHERE entity_type = 'ExpenseStatus' AND from_state = 'SupervisorApproved' AND to_state = 'Cancelled'
);

-- FinanceApproved → Cancelled
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects, sort_order)
SELECT 'ExpenseStatus', 'FinanceApproved', 'Cancelled', NULL, NULL, '[]'::jsonb, 33
WHERE NOT EXISTS (
    SELECT 1 FROM state_transition_defs WHERE entity_type = 'ExpenseStatus' AND from_state = 'FinanceApproved' AND to_state = 'Cancelled'
);

COMMIT;
