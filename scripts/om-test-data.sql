-- OM (Outsourcing) Test Data
-- Requires: suppliers, products, warehouses already exist
-- Enums: OutsourcingType (1=Full,2=Process,3=Material,4=Rework)
--        OutsourcingStatus (1=Draft,2=Sent,3=InProduction,4=Delivered,5=Received,6=Closed,7=ConvertedToInternal,8=Cancelled)
--        TrackingNodeType (0=SendMaterial,1=CarrierPickup,2=SupplierReceived,3=InProduction,4=Shipped,5=IqcInspected,6=Warehoused)

-- Use virtual warehouse "委外仓" id=23322
-- Suppliers: 1,2,4
-- Products: 565,566,567,568,569

INSERT INTO outsourcing_orders (doc_number, work_order_id, supplier_id, product_id, outsourcing_type, planned_qty, completed_qty, unit_price, scheduled_date, status, virtual_warehouse_id, remark, operator_id)
VALUES
  ('OM-2026-0001', NULL, 1, 565, 1, 1000.000000, 500.000000, 15.500000, '2026-06-15', 3, 23322, '全委外-生产中', 1),
  ('OM-2026-0002', NULL, 2, 566, 2, 2000.000000, 0.000000, 8.250000, '2026-06-20', 2, 23322, '工序委外-已发送', 1),
  ('OM-2026-0003', NULL, 1, 567, 3, 5000.000000, 5000.000000, 3.000000, '2026-06-01', 6, 23322, '材料委外-已关闭', 1),
  ('OM-2026-0004', NULL, 4, 568, 1, 800.000000, 0.000000, 22.000000, '2026-06-25', 1, 23322, '全委外-草稿', 1),
  ('OM-2026-0005', NULL, 2, 569, 4, 300.000000, 300.000000, 45.000000, '2026-05-20', 5, 23322, '委外返工-已收货', 1),
  ('OM-2026-0006', NULL, 1, 565, 2, 1500.000000, 750.000000, 12.000000, '2026-06-10', 4, 23322, '工序委外-已交付', 1),
  ('OM-2026-0007', NULL, 4, 566, 1, 2000.000000, 0.000000, 9.500000, '2026-07-01', 1, 23322, '全委外-草稿2', 1)
ON CONFLICT DO NOTHING;

-- Get the IDs for the inserted orders
-- OM-2026-0001 ~ id=1, OM-2026-0002 ~ id=2, etc. (may vary, use subquery)

-- Tracking records for OM-2026-0001 (status=3 InProduction - should have nodes 0,1,2,3 completed)
INSERT INTO outsourcing_trackings (outsourcing_id, node_type, tracked_at, planned_at, remark, operator_id)
SELECT o.id, 0, '2026-06-02 09:00:00+08', '2026-06-02', '发料完成', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001'
UNION ALL
SELECT o.id, 1, '2026-06-02 14:00:00+08', '2026-06-03', '承运商已取件', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001'
UNION ALL
SELECT o.id, 2, '2026-06-03 10:00:00+08', '2026-06-04', '供应商确认收货', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001'
UNION ALL
SELECT o.id, 3, '2026-06-04 08:00:00+08', '2026-06-05', '开始生产', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001';

-- Tracking for OM-2026-0002 (status=2 Sent - should have nodes 0,1 completed)
INSERT INTO outsourcing_trackings (outsourcing_id, node_type, tracked_at, planned_at, remark, operator_id)
SELECT o.id, 0, '2026-06-08 09:00:00+08', '2026-06-08', '发料完成', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0002'
UNION ALL
SELECT o.id, 1, '2026-06-08 16:00:00+08', '2026-06-09', '承运商已取件', 1 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0002';

-- Tracking for OM-2026-0003 (status=6 Closed - all nodes completed)
INSERT INTO outsourcing_trackings (outsourcing_id, node_type, tracked_at, planned_at, remark, operator_id)
SELECT o.id, n, 
  ('2026-05-10 ' || (8 + n*2) || ':00:00+08')::timestamptz,
  ('2026-05-' || (10 + n))::date,
  '节点' || n || '完成', 1 
FROM outsourcing_orders o, generate_series(0,6) n WHERE o.doc_number = 'OM-2026-0003';

-- Tracking for OM-2026-0005 (status=5 Received - nodes 0-5 completed)
INSERT INTO outsourcing_trackings (outsourcing_id, node_type, tracked_at, planned_at, remark, operator_id)
SELECT o.id, n,
  ('2026-05-15 ' || (8 + n*3) || ':00:00+08')::timestamptz,
  ('2026-05-' || (15 + n))::date,
  '节点' || n || '完成', 1
FROM outsourcing_orders o, generate_series(0,5) n WHERE o.doc_number = 'OM-2026-0005';

-- Tracking for OM-2026-0006 (status=4 Delivered - nodes 0-4 completed)
INSERT INTO outsourcing_trackings (outsourcing_id, node_type, tracked_at, planned_at, remark, operator_id)
SELECT o.id, n,
  ('2026-06-01 ' || (8 + n*2) || ':00:00+08')::timestamptz,
  ('2026-06-' || lpad((1 + n)::text, 2, '0'))::date,
  '节点' || n || '完成', 1
FROM outsourcing_orders o, generate_series(0,4) n WHERE o.doc_number = 'OM-2026-0006';

-- Materials for OM-2026-0001
INSERT INTO outsourcing_materials (outsourcing_id, product_id, planned_qty, sent_qty, returned_qty, unit_cost)
SELECT o.id, 568, 100.000000, 100.000000, 0.000000, 2.500000 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001'
UNION ALL
SELECT o.id, 569, 200.000000, 200.000000, 50.000000, 1.000000 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0001';

-- Materials for OM-2026-0002
INSERT INTO outsourcing_materials (outsourcing_id, product_id, planned_qty, sent_qty, returned_qty, unit_cost)
SELECT o.id, 565, 500.000000, 0.000000, 0.000000, 3.000000 FROM outsourcing_orders o WHERE o.doc_number = 'OM-2026-0002';
