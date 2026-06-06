-- ============================================================================
-- WMS 测试数据脚本
-- 目的：为 WMS 模块全面测试提供覆盖所有状态转换的测试数据
-- 前缀约定：所有测试数据编码以 WMS-TEST- 开头，便于识别和清理
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. 测试专用仓库（4 种类型）
-- ============================================================================

INSERT INTO warehouses (code, name, warehouse_type, status, address, manager_id, is_virtual, remark, operator_id)
VALUES
    ('WMS-TEST-WH-RAW',  '测试-原材料仓',  1, 1, 'A栋1楼', 1, false, 'WMS测试专用-原材料仓', 1),
    ('WMS-TEST-WH-FG',   '测试-成品仓',    2, 1, 'B栋1楼', 1, false, 'WMS测试专用-成品仓', 1),
    ('WMS-TEST-WH-SF',   '测试-半成品仓',  3, 1, 'C栋1楼', 1, false, 'WMS测试专用-半成品仓', 1),
    ('WMS-TEST-WH-VIRT', '测试-虚拟外包仓', 5, 1, NULL,     1, true,  'WMS测试专用-虚拟外包仓', 1)
ON CONFLICT (code) DO NOTHING;

-- ============================================================================
-- 2. 测试专用库区（每个仓库 2-3 个）
-- ============================================================================

INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
SELECT w.id, 'WMS-TEST-Z-RCV',  '测试收货区', 1, 1, '测试用收货区'
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
SELECT w.id, 'WMS-TEST-Z-STO',  '测试存储区', 2, 2, '测试用存储区'
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
SELECT w.id, 'WMS-TEST-Z-PIC',  '测试拣货区', 3, 3, '测试用拣货区'
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (warehouse_id, code) DO NOTHING;

-- 成品仓
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
SELECT w.id, 'WMS-TEST-Z-FG-RCV', '测试成品收货区', 1, 1, '成品入库区'
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-FG'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
SELECT w.id, 'WMS-TEST-Z-FG-STO', '测试成品存储区', 2, 2, '成品存储区'
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-FG'
ON CONFLICT (warehouse_id, code) DO NOTHING;

-- 半成品仓
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order, remark)
SELECT w.id, 'WMS-TEST-Z-SF-STO', '测试半成品存储区', 2, 1, '半成品存储区'
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-SF'
ON CONFLICT (warehouse_id, code) DO NOTHING;

-- ============================================================================
-- 3. 测试专用储位（每个存储区 3 个）
-- ============================================================================

-- 原材料仓存储区储位
INSERT INTO bins (zone_id, code, name, row_no, column_no, layer_no, capacity_limit, status)
SELECT z.id, 'WMS-TEST-BIN-R01', '测试储位-R01', 'R1', 'C1', 'L1', 1000.0, 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO'
ON CONFLICT (zone_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, row_no, column_no, layer_no, capacity_limit, status)
SELECT z.id, 'WMS-TEST-BIN-R02', '测试储位-R02', 'R1', 'C2', 'L1', 1000.0, 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO'
ON CONFLICT (zone_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, row_no, column_no, layer_no, capacity_limit, status)
SELECT z.id, 'WMS-TEST-BIN-R03', '测试储位-R03', 'R2', 'C1', 'L1', 1000.0, 2
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO'
ON CONFLICT (zone_id, code) DO NOTHING;

-- 成品仓存储区储位
INSERT INTO bins (zone_id, code, name, row_no, column_no, layer_no, capacity_limit, status)
SELECT z.id, 'WMS-TEST-BIN-F01', '测试储位-F01', 'R1', 'C1', 'L1', 500.0, 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WMS-TEST-WH-FG' AND z.code = 'WMS-TEST-Z-FG-STO'
ON CONFLICT (zone_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, row_no, column_no, layer_no, capacity_limit, status)
SELECT z.id, 'WMS-TEST-BIN-F02', '测试储位-F02', 'R1', 'C2', 'L1', 500.0, 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WMS-TEST-WH-FG' AND z.code = 'WMS-TEST-Z-FG-STO'
ON CONFLICT (zone_id, code) DO NOTHING;

-- 半成品仓储位
INSERT INTO bins (zone_id, code, name, row_no, column_no, layer_no, capacity_limit, status)
SELECT z.id, 'WMS-TEST-BIN-S01', '测试储位-S01', 'R1', 'C1', 'L1', 800.0, 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WMS-TEST-WH-SF' AND z.code = 'WMS-TEST-Z-SF-STO'
ON CONFLICT (zone_id, code) DO NOTHING;

-- ============================================================================
-- 4. 库存数据（stock_ledger）— 使用已有产品
-- ============================================================================

-- 在原材料仓 R01 储位放 3 个产品的库存
INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, reserved_qty, available_qty, unit_cost, received_date)
SELECT p.product_id, w.id, z.id, b.id, 'WMS-TEST-B001', 100.0, 0, 100.0, 10.50, CURRENT_DATE
FROM products p, warehouses w, zones z, bins b
WHERE p.product_id = 565
  AND w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO' AND b.code = 'WMS-TEST-BIN-R01'
ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, '')) DO NOTHING;

INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, reserved_qty, available_qty, unit_cost, received_date)
SELECT p.product_id, w.id, z.id, b.id, 'WMS-TEST-B002', 200.0, 0, 200.0, 25.00, CURRENT_DATE
FROM products p, warehouses w, zones z, bins b
WHERE p.product_id = 566
  AND w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO' AND b.code = 'WMS-TEST-BIN-R01'
ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, '')) DO NOTHING;

INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, reserved_qty, available_qty, unit_cost, received_date)
SELECT p.product_id, w.id, z.id, b.id, 'WMS-TEST-B003', 50.0, 10.0, 40.0, 8.00, CURRENT_DATE
FROM products p, warehouses w, zones z, bins b
WHERE p.product_id = 567
  AND w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO' AND b.code = 'WMS-TEST-BIN-R02'
ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, '')) DO NOTHING;

-- 在成品仓放库存
INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, reserved_qty, available_qty, unit_cost, received_date)
SELECT p.product_id, w.id, z.id, b.id, 'WMS-TEST-B010', 80.0, 0, 80.0, 55.00, CURRENT_DATE
FROM products p, warehouses w, zones z, bins b
WHERE p.product_id = 568
  AND w.code = 'WMS-TEST-WH-FG' AND z.code = 'WMS-TEST-Z-FG-STO' AND b.code = 'WMS-TEST-BIN-F01'
ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, '')) DO NOTHING;

-- ============================================================================
-- 5. 库存调拨单（各状态）
-- ============================================================================

-- Draft 调拨单
INSERT INTO inventory_transfers (doc_number, from_warehouse_id, to_warehouse_id, from_zone_id, to_zone_id, from_bin_id, to_bin_id, transfer_date, status, operator_id)
SELECT 'WMS-TEST-TF-DRAFT', w1.id, w2.id, z1.id, z2.id, b1.id, b2.id, CURRENT_DATE, 1, 1
FROM warehouses w1, warehouses w2, zones z1, zones z2, bins b1, bins b2
WHERE w1.code = 'WMS-TEST-WH-RAW' AND w2.code = 'WMS-TEST-WH-FG'
  AND z1.code = 'WMS-TEST-Z-STO' AND z2.code = 'WMS-TEST-Z-FG-STO'
  AND b1.code = 'WMS-TEST-BIN-R01' AND b2.code = 'WMS-TEST-BIN-F01'
ON CONFLICT (doc_number) DO NOTHING;

-- Draft 调拨单的行项目
INSERT INTO transfer_items (transfer_id, product_id, quantity, batch_no)
SELECT t.id, 565, 20.0, 'WMS-TEST-B001'
FROM inventory_transfers t WHERE t.doc_number = 'WMS-TEST-TF-DRAFT'
ON CONFLICT DO NOTHING;

-- InTransit 调拨单
INSERT INTO inventory_transfers (doc_number, from_warehouse_id, to_warehouse_id, from_zone_id, to_zone_id, from_bin_id, to_bin_id, transfer_date, status, operator_id)
SELECT 'WMS-TEST-TF-TRANSIT', w1.id, w2.id, z1.id, z2.id, b1.id, b2.id, CURRENT_DATE, 2, 1
FROM warehouses w1, warehouses w2, zones z1, zones z2, bins b1, bins b2
WHERE w1.code = 'WMS-TEST-WH-RAW' AND w2.code = 'WMS-TEST-WH-SF'
  AND z1.code = 'WMS-TEST-Z-STO' AND z2.code = 'WMS-TEST-Z-SF-STO'
  AND b1.code = 'WMS-TEST-BIN-R02' AND b2.code = 'WMS-TEST-BIN-S01'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO transfer_items (transfer_id, product_id, quantity, batch_no)
SELECT t.id, 566, 30.0, 'WMS-TEST-B002'
FROM inventory_transfers t WHERE t.doc_number = 'WMS-TEST-TF-TRANSIT'
ON CONFLICT DO NOTHING;

-- Completed 调拨单
INSERT INTO inventory_transfers (doc_number, from_warehouse_id, to_warehouse_id, from_zone_id, to_zone_id, from_bin_id, to_bin_id, transfer_date, status, operator_id)
SELECT 'WMS-TEST-TF-DONE', w1.id, w2.id, z1.id, z2.id, b1.id, b2.id, CURRENT_DATE - INTERVAL '3 days', 3, 1
FROM warehouses w1, warehouses w2, zones z1, zones z2, bins b1, bins b2
WHERE w1.code = 'WMS-TEST-WH-FG' AND w2.code = 'WMS-TEST-WH-RAW'
  AND z1.code = 'WMS-TEST-Z-FG-STO' AND z2.code = 'WMS-TEST-Z-STO'
  AND b1.code = 'WMS-TEST-BIN-F02' AND b2.code = 'WMS-TEST-BIN-R03'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO transfer_items (transfer_id, product_id, quantity, batch_no)
SELECT t.id, 568, 10.0, NULL
FROM inventory_transfers t WHERE t.doc_number = 'WMS-TEST-TF-DONE'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- 6. 形态转换单（各状态）
-- ============================================================================

-- Draft 转换单
INSERT INTO form_conversions (doc_number, warehouse_id, conversion_date, status, remark, operator_id)
SELECT 'WMS-TEST-FC-DRAFT', w.id, CURRENT_DATE, 1, '测试形态转换-草稿', 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

-- 消耗项
INSERT INTO conversion_items (conversion_id, direction, product_id, quantity, unit_cost, batch_no)
SELECT fc.id, 1, 565, 10.0, 10.50, 'WMS-TEST-B001'
FROM form_conversions fc WHERE fc.doc_number = 'WMS-TEST-FC-DRAFT'
ON CONFLICT DO NOTHING;

-- 产出项
INSERT INTO conversion_items (conversion_id, direction, product_id, quantity, unit_cost, batch_no)
SELECT fc.id, 2, 568, 5.0, 25.00, NULL
FROM form_conversions fc WHERE fc.doc_number = 'WMS-TEST-FC-DRAFT'
ON CONFLICT DO NOTHING;

-- Completed 转换单
INSERT INTO form_conversions (doc_number, warehouse_id, conversion_date, status, remark, operator_id)
SELECT 'WMS-TEST-FC-DONE', w.id, CURRENT_DATE - INTERVAL '2 days', 2, '测试形态转换-已完成', 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO conversion_items (conversion_id, direction, product_id, quantity, unit_cost, batch_no)
SELECT fc.id, 1, 566, 20.0, 25.00, 'WMS-TEST-B002'
FROM form_conversions fc WHERE fc.doc_number = 'WMS-TEST-FC-DONE'
ON CONFLICT DO NOTHING;

INSERT INTO conversion_items (conversion_id, direction, product_id, quantity, unit_cost, batch_no)
SELECT fc.id, 2, 569, 8.0, 60.00, NULL
FROM form_conversions fc WHERE fc.doc_number = 'WMS-TEST-FC-DONE'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- 7. 循环盘点单（各状态）
-- ============================================================================

-- Draft 盘点单
INSERT INTO cycle_counts (doc_number, warehouse_id, zone_id, count_date, status, is_blind, remark, operator_id)
SELECT 'WMS-TEST-CC-DRAFT', w.id, z.id, CURRENT_DATE, 1, false, '测试盘点-草稿', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO cycle_count_items (count_id, bin_id, product_id, batch_no, system_qty, counted_qty, variance_qty)
SELECT cc.id, b.id, 565, 'WMS-TEST-B001', 100.0, 0, 0
FROM cycle_counts cc, bins b
WHERE cc.doc_number = 'WMS-TEST-CC-DRAFT' AND b.code = 'WMS-TEST-BIN-R01'
ON CONFLICT DO NOTHING;

-- Counting 盘点单
INSERT INTO cycle_counts (doc_number, warehouse_id, zone_id, count_date, status, is_blind, remark, operator_id)
SELECT 'WMS-TEST-CC-COUNTING', w.id, z.id, CURRENT_DATE - INTERVAL '1 day', 2, true, '测试盘点-盘点中', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO cycle_count_items (count_id, bin_id, product_id, batch_no, system_qty, counted_qty, variance_qty)
SELECT cc.id, b.id, 566, 'WMS-TEST-B002', 200.0, 0, 0
FROM cycle_counts cc, bins b
WHERE cc.doc_number = 'WMS-TEST-CC-COUNTING' AND b.code = 'WMS-TEST-BIN-R01'
ON CONFLICT DO NOTHING;

-- Completed 盘点单
INSERT INTO cycle_counts (doc_number, warehouse_id, zone_id, count_date, status, is_blind, remark, operator_id)
SELECT 'WMS-TEST-CC-COMPLETE', w.id, z.id, CURRENT_DATE - INTERVAL '2 days', 3, false, '测试盘点-已完成', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO cycle_count_items (count_id, bin_id, product_id, batch_no, system_qty, counted_qty, variance_qty)
SELECT cc.id, b.id, 567, 'WMS-TEST-B003', 50.0, 48.0, -2.0
FROM cycle_counts cc, bins b
WHERE cc.doc_number = 'WMS-TEST-CC-COMPLETE' AND b.code = 'WMS-TEST-BIN-R02'
ON CONFLICT DO NOTHING;

-- Adjusted 盘点单
INSERT INTO cycle_counts (doc_number, warehouse_id, zone_id, count_date, status, is_blind, remark, operator_id)
SELECT 'WMS-TEST-CC-ADJUSTED', w.id, z.id, CURRENT_DATE - INTERVAL '5 days', 4, false, '测试盘点-已调整', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-FG' AND z.code = 'WMS-TEST-Z-FG-STO'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO cycle_count_items (count_id, bin_id, product_id, batch_no, system_qty, counted_qty, variance_qty, is_adjusted)
SELECT cc.id, b.id, 568, 'WMS-TEST-B010', 80.0, 78.0, -2.0, true
FROM cycle_counts cc, bins b
WHERE cc.doc_number = 'WMS-TEST-CC-ADJUSTED' AND b.code = 'WMS-TEST-BIN-F01'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- 8. 库存锁定（各状态）
-- ============================================================================

-- Active 锁库单
INSERT INTO inventory_locks (doc_number, product_id, warehouse_id, locked_qty, lock_reason, customer_id, status, operator_id)
SELECT 'WMS-TEST-LK-ACTIVE', 565, w.id, 10.0, '客户订单预留', NULL, 1, 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

-- Released 锁库单
INSERT INTO inventory_locks (doc_number, product_id, warehouse_id, locked_qty, lock_reason, customer_id, status, operator_id)
SELECT 'WMS-TEST-LK-RELEASED', 566, w.id, 15.0, '已释放锁定', NULL, 2, 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

-- Cancelled 锁库单
INSERT INTO inventory_locks (doc_number, product_id, warehouse_id, locked_qty, lock_reason, customer_id, status, operator_id)
SELECT 'WMS-TEST-LK-CANCELLED', 567, w.id, 5.0, '已取消锁定', NULL, 3, 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

-- ============================================================================
-- 9. 来料通知（各状态）
-- ============================================================================

-- Draft 来料通知
INSERT INTO arrival_notices (doc_number, purchase_order_id, supplier_id, arrival_date, status, warehouse_id, zone_id, delivery_note, remark, operator_id)
SELECT 'WMS-TEST-AN-DRAFT', NULL, 1, CURRENT_DATE, 1, w.id, z.id, 'DN-TEST-001', '测试来料通知-草稿', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-RCV'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO arrival_notice_items (notice_id, order_item_id, product_id, declared_qty, received_qty, accepted_qty, batch_no)
SELECT an.id, NULL, 565, 100.0, 0, 0, NULL
FROM arrival_notices an WHERE an.doc_number = 'WMS-TEST-AN-DRAFT'
ON CONFLICT DO NOTHING;

INSERT INTO arrival_notice_items (notice_id, order_item_id, product_id, declared_qty, received_qty, accepted_qty, batch_no)
SELECT an.id, NULL, 566, 50.0, 0, 0, NULL
FROM arrival_notices an WHERE an.doc_number = 'WMS-TEST-AN-DRAFT'
ON CONFLICT DO NOTHING;

-- Received 来料通知
INSERT INTO arrival_notices (doc_number, purchase_order_id, supplier_id, arrival_date, status, warehouse_id, zone_id, delivery_note, remark, operator_id)
SELECT 'WMS-TEST-AN-RECEIVED', NULL, 2, CURRENT_DATE - INTERVAL '1 day', 2, w.id, z.id, 'DN-TEST-002', '测试来料通知-已收货', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-RCV'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO arrival_notice_items (notice_id, order_item_id, product_id, declared_qty, received_qty, accepted_qty, batch_no)
SELECT an.id, NULL, 567, 80.0, 80.0, 0, 'WMS-TEST-B020'
FROM arrival_notices an WHERE an.doc_number = 'WMS-TEST-AN-RECEIVED'
ON CONFLICT DO NOTHING;

-- Inspecting 来料通知
INSERT INTO arrival_notices (doc_number, purchase_order_id, supplier_id, arrival_date, status, warehouse_id, zone_id, delivery_note, remark, operator_id)
SELECT 'WMS-TEST-AN-INSPECTING', NULL, 1, CURRENT_DATE - INTERVAL '2 days', 3, w.id, z.id, 'DN-TEST-003', '测试来料通知-检验中', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-RCV'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO arrival_notice_items (notice_id, order_item_id, product_id, declared_qty, received_qty, accepted_qty, batch_no)
SELECT an.id, NULL, 568, 60.0, 60.0, 0, 'WMS-TEST-B030'
FROM arrival_notices an WHERE an.doc_number = 'WMS-TEST-AN-INSPECTING'
ON CONFLICT DO NOTHING;

-- Accepted 来料通知
INSERT INTO arrival_notices (doc_number, purchase_order_id, supplier_id, arrival_date, status, warehouse_id, zone_id, delivery_note, remark, operator_id)
SELECT 'WMS-TEST-AN-ACCEPTED', NULL, 2, CURRENT_DATE - INTERVAL '5 days', 4, w.id, z.id, 'DN-TEST-004', '测试来料通知-已接收', 1
FROM warehouses w, zones z
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-RCV'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO arrival_notice_items (notice_id, order_item_id, product_id, declared_qty, received_qty, accepted_qty, batch_no)
SELECT an.id, NULL, 569, 40.0, 40.0, 40.0, 'WMS-TEST-B040'
FROM arrival_notices an WHERE an.doc_number = 'WMS-TEST-AN-ACCEPTED'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- 10. 领料单（各状态）
-- ============================================================================

-- Draft 领料单（work_order_id 用 0 占位，无实际工单）
INSERT INTO material_requisitions (doc_number, work_order_id, requisition_date, status, warehouse_id, operator_id)
SELECT 'WMS-TEST-MR-DRAFT', 0, CURRENT_DATE, 1, w.id, 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO material_requisition_items (requisition_id, product_id, requested_qty, issued_qty, variance_qty, bin_id)
SELECT mr.id, 565, 30.0, 0, 0, NULL
FROM material_requisitions mr WHERE mr.doc_number = 'WMS-TEST-MR-DRAFT'
ON CONFLICT DO NOTHING;

-- Confirmed 领料单
INSERT INTO material_requisitions (doc_number, work_order_id, requisition_date, status, warehouse_id, operator_id)
SELECT 'WMS-TEST-MR-CONFIRMED', 0, CURRENT_DATE - INTERVAL '1 day', 2, w.id, 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO material_requisition_items (requisition_id, product_id, requested_qty, issued_qty, variance_qty, bin_id)
SELECT mr.id, 566, 25.0, 0, 0, NULL
FROM material_requisitions mr WHERE mr.doc_number = 'WMS-TEST-MR-CONFIRMED'
ON CONFLICT DO NOTHING;

INSERT INTO material_requisition_items (requisition_id, product_id, requested_qty, issued_qty, variance_qty, bin_id)
SELECT mr.id, 567, 15.0, 0, 0, NULL
FROM material_requisitions mr WHERE mr.doc_number = 'WMS-TEST-MR-CONFIRMED'
ON CONFLICT DO NOTHING;

-- Issued 领料单
INSERT INTO material_requisitions (doc_number, work_order_id, requisition_date, status, warehouse_id, operator_id)
SELECT 'WMS-TEST-MR-ISSUED', 0, CURRENT_DATE - INTERVAL '3 days', 3, w.id, 1
FROM warehouses w WHERE w.code = 'WMS-TEST-WH-RAW'
ON CONFLICT (doc_number) DO NOTHING;

INSERT INTO material_requisition_items (requisition_id, product_id, requested_qty, issued_qty, variance_qty, bin_id)
SELECT mr.id, 565, 20.0, 20.0, 0, NULL
FROM material_requisitions mr WHERE mr.doc_number = 'WMS-TEST-MR-ISSUED'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- 11. 库存事务记录（入库/出库/调拨等）
-- ============================================================================

INSERT INTO inventory_transactions (doc_number, transaction_type, product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, unit_cost, source_type, source_id, remark, operator_id)
SELECT 'WMS-TEST-IT-001', 1, 565, w.id, z.id, b.id, 'WMS-TEST-B001', 100.0, 10.50, 'manual', 0, '测试入库事务', 1
FROM warehouses w, zones z, bins b
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO' AND b.code = 'WMS-TEST-BIN-R01'
ON CONFLICT DO NOTHING;

INSERT INTO inventory_transactions (doc_number, transaction_type, product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, unit_cost, source_type, source_id, remark, operator_id)
SELECT 'WMS-TEST-IT-002', 3, 568, w.id, z.id, b.id, 'WMS-TEST-B010', 5.0, 55.00, 'manual', 0, '测试出库事务', 1
FROM warehouses w, zones z, bins b
WHERE w.code = 'WMS-TEST-WH-FG' AND z.code = 'WMS-TEST-Z-FG-STO' AND b.code = 'WMS-TEST-BIN-F01'
ON CONFLICT DO NOTHING;

INSERT INTO inventory_transactions (doc_number, transaction_type, product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, unit_cost, source_type, source_id, remark, operator_id)
SELECT 'WMS-TEST-IT-003', 7, 566, w.id, z.id, b.id, 'WMS-TEST-B002', 10.0, 25.00, 'transfer', 0, '测试调拨事务', 1
FROM warehouses w, zones z, bins b
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO' AND b.code = 'WMS-TEST-BIN-R01'
ON CONFLICT DO NOTHING;

INSERT INTO inventory_transactions (doc_number, transaction_type, product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, unit_cost, source_type, source_id, remark, operator_id)
SELECT 'WMS-TEST-IT-004', 10, 565, w.id, z.id, b.id, 'WMS-TEST-B001', 10.0, 0, 'lock', 0, '测试锁定事务', 1
FROM warehouses w, zones z, bins b
WHERE w.code = 'WMS-TEST-WH-RAW' AND z.code = 'WMS-TEST-Z-STO' AND b.code = 'WMS-TEST-BIN-R01'
ON CONFLICT DO NOTHING;

COMMIT;

-- ============================================================================
-- 验证查询
-- ============================================================================

SELECT 'warehouses' AS tbl, COUNT(*) AS cnt FROM warehouses WHERE code LIKE 'WMS-TEST-%' AND deleted_at IS NULL
UNION ALL
SELECT 'zones', COUNT(*) FROM zones WHERE code LIKE 'WMS-TEST-%' AND deleted_at IS NULL
UNION ALL
SELECT 'bins', COUNT(*) FROM bins WHERE code LIKE 'WMS-TEST-%' AND deleted_at IS NULL
UNION ALL
SELECT 'stock_ledger', COUNT(*) FROM stock_ledger sl JOIN bins b ON sl.bin_id = b.id WHERE b.code LIKE 'WMS-TEST-%'
UNION ALL
SELECT 'transfers', COUNT(*) FROM inventory_transfers WHERE doc_number LIKE 'WMS-TEST-%'
UNION ALL
SELECT 'conversions', COUNT(*) FROM form_conversions WHERE doc_number LIKE 'WMS-TEST-%'
UNION ALL
SELECT 'cycle_counts', COUNT(*) FROM cycle_counts WHERE doc_number LIKE 'WMS-TEST-%'
UNION ALL
SELECT 'locks', COUNT(*) FROM inventory_locks WHERE doc_number LIKE 'WMS-TEST-%'
UNION ALL
SELECT 'arrivals', COUNT(*) FROM arrival_notices WHERE doc_number LIKE 'WMS-TEST-%' AND deleted_at IS NULL
UNION ALL
SELECT 'requisitions', COUNT(*) FROM material_requisitions WHERE doc_number LIKE 'WMS-TEST-%' AND deleted_at IS NULL
UNION ALL
SELECT 'transactions', COUNT(*) FROM inventory_transactions WHERE doc_number LIKE 'WMS-TEST-%';
