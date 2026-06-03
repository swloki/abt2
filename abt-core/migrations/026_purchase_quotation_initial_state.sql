-- Add '' -> 'Draft' transitions for entities whose create() logs initial state
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('PurchaseQuotation', '', 'Draft', NULL, 0),
    ('MiscellaneousRequest', '', 'Draft', NULL, 0),
    ('PurchaseOrder', '', 'Draft', NULL, 0),
    ('PaymentRequest', '', 'Draft', NULL, 0),
    ('PurchaseReconciliation', '', 'Draft', NULL, 0),
    ('PurchaseReturn', '', 'Draft', NULL, 0)
ON CONFLICT DO NOTHING;

-- Backfill existing records that have no state log
INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'PurchaseQuotation', pq.id, '', 'Draft', t.id, COALESCE(pq.operator_id, 0), 'backfill'
FROM purchase_quotations pq
JOIN state_transition_defs t ON t.entity_type = 'PurchaseQuotation' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE NOT EXISTS (SELECT 1 FROM entity_state_logs esl WHERE esl.entity_type = 'PurchaseQuotation' AND esl.entity_id = pq.id)
ON CONFLICT DO NOTHING;

INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'PurchaseOrder', po.id, '', 'Draft', t.id, COALESCE(po.operator_id, 0), 'backfill'
FROM purchase_orders po
JOIN state_transition_defs t ON t.entity_type = 'PurchaseOrder' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE NOT EXISTS (SELECT 1 FROM entity_state_logs esl WHERE esl.entity_type = 'PurchaseOrder' AND esl.entity_id = po.id)
ON CONFLICT DO NOTHING;

INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'MiscellaneousRequest', mr.id, '', 'Draft', t.id, COALESCE(mr.operator_id, 0), 'backfill'
FROM miscellaneous_requests mr
JOIN state_transition_defs t ON t.entity_type = 'MiscellaneousRequest' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE NOT EXISTS (SELECT 1 FROM entity_state_logs esl WHERE esl.entity_type = 'MiscellaneousRequest' AND esl.entity_id = mr.id)
ON CONFLICT DO NOTHING;

INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'PaymentRequest', pr.id, '', 'Draft', t.id, COALESCE(pr.operator_id, 0), 'backfill'
FROM payment_requests pr
JOIN state_transition_defs t ON t.entity_type = 'PaymentRequest' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE NOT EXISTS (SELECT 1 FROM entity_state_logs esl WHERE esl.entity_type = 'PaymentRequest' AND esl.entity_id = pr.id)
ON CONFLICT DO NOTHING;

INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'PurchaseReconciliation', pr.id, '', 'Draft', t.id, COALESCE(pr.operator_id, 0), 'backfill'
FROM purchase_reconciliations pr
JOIN state_transition_defs t ON t.entity_type = 'PurchaseReconciliation' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE NOT EXISTS (SELECT 1 FROM entity_state_logs esl WHERE esl.entity_type = 'PurchaseReconciliation' AND esl.entity_id = pr.id)
ON CONFLICT DO NOTHING;

INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'PurchaseReturn', pr.id, '', 'Draft', t.id, COALESCE(pr.operator_id, 0), 'backfill'
FROM purchase_returns pr
JOIN state_transition_defs t ON t.entity_type = 'PurchaseReturn' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE NOT EXISTS (SELECT 1 FROM entity_state_logs esl WHERE esl.entity_type = 'PurchaseReturn' AND esl.entity_id = pr.id)
ON CONFLICT DO NOTHING;
