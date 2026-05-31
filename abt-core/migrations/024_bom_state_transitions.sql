-- BOM state transition definitions
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('BomStatus', '', 'Draft', NULL, 1),
    ('BomStatus', 'Draft', 'Published', NULL, 2),
    ('BomStatus', 'Published', 'Draft', NULL, 3)
ON CONFLICT DO NOTHING;

-- Backfill: 为所有现有 BOM 补上状态日志（创建时因无转换规则，初始状态写入被 .ok() 静默吞掉）
-- Draft BOMs (status=1): '' -> Draft
INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'BomStatus', bom_id, '', 'Draft', t.id, COALESCE(b.created_by, 0), 'backfill'
FROM boms b
JOIN state_transition_defs t ON t.entity_type = 'BomStatus' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE b.status = 1 AND b.deleted_at IS NULL
ON CONFLICT DO NOTHING;

-- Published BOMs (status=2): '' -> Draft
INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'BomStatus', bom_id, '', 'Draft', t.id, COALESCE(b.created_by, 0), 'backfill'
FROM boms b
JOIN state_transition_defs t ON t.entity_type = 'BomStatus' AND t.from_state = '' AND t.to_state = 'Draft'
WHERE b.status = 2 AND b.deleted_at IS NULL
ON CONFLICT DO NOTHING;

-- Published BOMs (status=2): Draft -> Published
INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
SELECT 'BomStatus', bom_id, 'Draft', 'Published', t.id, COALESCE(b.created_by, 0), 'backfill'
FROM boms b
JOIN state_transition_defs t ON t.entity_type = 'BomStatus' AND t.from_state = 'Draft' AND t.to_state = 'Published'
WHERE b.status = 2 AND b.deleted_at IS NULL
ON CONFLICT DO NOTHING;
