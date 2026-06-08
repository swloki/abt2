-- Fix tracking for OM-2026-0001
INSERT INTO outsourcing_trackings (outsourcing_id, node_type, tracked_at, planned_at, remark, operator_id)
SELECT o.id, 0, '2026-06-02 09:00:00+08'::timestamptz, '2026-06-02'::date, 'send material done', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001'
UNION ALL
SELECT o.id, 1, '2026-06-02 14:00:00+08'::timestamptz, '2026-06-03'::date, 'carrier pickup', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001'
UNION ALL
SELECT o.id, 2, '2026-06-03 10:00:00+08'::timestamptz, '2026-06-04'::date, 'supplier received', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001'
UNION ALL
SELECT o.id, 3, '2026-06-04 08:00:00+08'::timestamptz, '2026-06-05'::date, 'in production', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001';

-- Fix tracking for OM-2026-0002
INSERT INTO outsourcing_trackings (outsourcing_id, node_type, tracked_at, planned_at, remark, operator_id)
SELECT o.id, 0, '2026-06-08 09:00:00+08'::timestamptz, '2026-06-08'::date, 'send material', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0002'
UNION ALL
SELECT o.id, 1, '2026-06-08 16:00:00+08'::timestamptz, '2026-06-09'::date, 'carrier pickup', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0002';
