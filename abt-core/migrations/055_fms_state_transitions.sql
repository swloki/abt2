-- FMS 状态机定义：ExpenseStatus + JournalStatus
-- 此前 expense.create / cash_journal.create / confirm 调 StateMachineService.transition()
-- 因 entity_type 未注册而 InvalidStateTransition（用 ? 传播，硬失败）。
-- 参照 024_bom_state_transitions.sql 格式。含初始转换 ('' -> X) 供新实体首转。

-- ── state_definitions ──
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('JournalStatus', 'Draft',     '草稿',    TRUE,  FALSE),
    ('JournalStatus', 'Confirmed', '已确认',  FALSE, FALSE),
    ('JournalStatus', 'Cancelled', '已取消',  FALSE, TRUE),
    ('ExpenseStatus', 'Draft',     '草稿',    TRUE,  FALSE),
    ('ExpenseStatus', 'Submitted', '已提交',  FALSE, FALSE),
    ('ExpenseStatus', 'Approved',  '已审批',  FALSE, FALSE),
    ('ExpenseStatus', 'Paid',      '已付款',  FALSE, TRUE),
    ('ExpenseStatus', 'Cancelled', '已取消',  FALSE, TRUE)
ON CONFLICT (entity_type, state_name) DO NOTHING;

-- ── state_transition_defs ──
-- 注：Approved -> Paid 不进状态机表，由 generate_payment_journal 内部直接 update_status 完成。
INSERT INTO state_transition_defs (entity_type, from_state, to_state, sort_order) VALUES
    ('JournalStatus', '',          'Draft',     1),
    ('JournalStatus', 'Draft',     'Confirmed', 2),
    ('JournalStatus', 'Draft',     'Cancelled', 3),
    ('ExpenseStatus', '',          'Draft',     1),
    ('ExpenseStatus', 'Draft',     'Submitted', 2),
    ('ExpenseStatus', 'Submitted', 'Approved',  3),
    ('ExpenseStatus', 'Submitted', 'Cancelled', 4),
    ('ExpenseStatus', 'Approved',  'Cancelled', 5)
ON CONFLICT (entity_type, from_state, to_state) DO NOTHING;
