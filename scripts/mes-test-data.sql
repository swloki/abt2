-- MES Test Data - corrected for actual schema

-- Add routings for existing work orders
INSERT INTO work_order_routings (work_order_id, step_no, process_name, planned_qty, completed_qty, defect_qty, status)
VALUES
(1, 1, 'SMD', 100, 50, 2, 2),
(1, 2, 'ASSY', 100, 0, 0, 1),
(1, 3, 'TEST', 100, 0, 0, 1),
(2, 1, 'SMD', 100, 100, 0, 3),
(2, 2, 'ASSY', 100, 50, 0, 2),
(3, 1, 'SMD', 200, 0, 0, 1)
ON CONFLICT (work_order_id, step_no) DO NOTHING;

-- Insert work reports
INSERT INTO work_reports (doc_number, work_order_id, batch_id, routing_id, report_date, shift, worker_id, completed_qty, defect_qty, work_hours, remark, operator_id, created_at)
SELECT 'WR-2026-06-0000' || gs.n, wo.id, pb.id, wor.id,
  CURRENT_DATE, 1, 1,
  CASE gs.n WHEN 2 THEN 100 WHEN 3 THEN 50 WHEN 4 THEN 98 WHEN 5 THEN 200 ELSE 50 END,
  CASE gs.n WHEN 2 THEN 2 WHEN 4 THEN 1 ELSE 0 END,
  8, '', 1,
  NOW() - (gs.n || ' hours')::interval
FROM generate_series(2,5) AS gs(n)
JOIN work_orders wo ON wo.id = gs.n - 1
JOIN production_batches pb ON pb.work_order_id = wo.id
JOIN work_order_routings wor ON wor.work_order_id = wo.id AND wor.step_no = 1
WHERE NOT EXISTS (SELECT 1 FROM work_reports wr WHERE wr.doc_number = 'WR-2026-06-0000' || gs.n);

-- Insert more inspections
INSERT INTO production_inspections (doc_number, work_order_id, routing_id, product_id, inspection_type, sample_qty, qualified_qty, unqualified_qty, result, inspector_id, inspection_date, disposition, remark, operator_id)
VALUES
('PI-2026-06-00003', 1, 1, 565, 2, 20, 19, 1, 1, 1, '2026-06-07', 'Accept', 'Inprocess check', 1),
('PI-2026-06-00004', 2, 2, 565, 3, 50, 50, 0, 1, 1, '2026-06-07', 'Accept', 'Final check pass', 1)
ON CONFLICT (doc_number) DO NOTHING;
