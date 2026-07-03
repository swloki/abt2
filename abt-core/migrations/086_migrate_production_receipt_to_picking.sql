-- 086: production_receipts → stock_pickings (IncomingWorkOrder) 数据迁移
-- #146 阶段 5b：生产入库迁入 stock_pickings(IncomingWorkOrder=2)
-- status 映射：receipt Draft(1)→Draft(1), Confirmed(2)→Done(3), Cancelled(3)→Cancelled(4)
--   （原 receipt.confirm 即终态入库完成；映射到 picking Done 语义）
-- 字段映射：work_order_id→source_id+work_order_id, product_id/received_qty/batch_id→items[0],
--   warehouse_id/zone_id/bin_id→to_*, receipt_date→scheduled_date
-- 幂等：NOT EXISTS doc_number 避免重复；production_receipts 表保留归档（阶段 6 DROP）
-- InspectionResult.source_id（旧 receipt.id）通过 doc_number 映射到新 picking.id（FQC 关联）

INSERT INTO stock_pickings
    (doc_number, picking_type, status, source_type, source_id, partner_id,
     to_warehouse_id, to_zone_id, to_bin_id,
     scheduled_date, done_at, work_order_id, remark, operator_id, created_at, updated_at, deleted_at)
SELECT
    pr.doc_number,
    2,  -- PickingType::IncomingWorkOrder
    CASE pr.status
        WHEN 1 THEN 1  -- Draft → Draft
        WHEN 2 THEN 3  -- Confirmed → Done（原 confirm 即入库完成）
        WHEN 3 THEN 4  -- Cancelled → Cancelled
    END AS status,
    'work_order',
    pr.work_order_id,  -- source_id
    NULL,
    pr.warehouse_id, pr.zone_id, pr.bin_id,  -- to_*
    pr.receipt_date,  -- scheduled_date
    CASE WHEN pr.status = 2 THEN pr.updated_at ELSE NULL END,  -- done_at（Confirmed 用 updated_at 兜底）
    pr.work_order_id,  -- work_order_id
    COALESCE(pr.remark, ''),
    pr.operator_id,
    pr.created_at,
    pr.updated_at,
    pr.deleted_at
FROM production_receipts pr
WHERE NOT EXISTS (
    SELECT 1 FROM stock_pickings sp
    WHERE sp.picking_type = 2 AND sp.doc_number = pr.doc_number
);

-- production_receipts 头表字段 → stock_picking_items（每张 receipt 一条 item）
INSERT INTO stock_picking_items
    (picking_id, product_id, batch_id, qty_requested, qty_done, remark, created_at)
SELECT
    sp.id,
    pr.product_id,
    pr.batch_id,
    pr.received_qty,  -- qty_requested
    CASE WHEN pr.status = 2 THEN pr.received_qty ELSE 0 END,  -- qty_done（Confirmed=已入库）
    '',
    pr.created_at
FROM production_receipts pr
JOIN stock_pickings sp ON sp.picking_type = 2 AND sp.doc_number = pr.doc_number
WHERE NOT EXISTS (
    SELECT 1 FROM stock_picking_items spi
    WHERE spi.picking_id = sp.id
);

-- InspectionResult.source_id 映射：旧 receipt.id → 新 picking.id（FQC 检验记录关联）
-- source_type = 5 (InspectionSourceType::ProductionReceipt)，picking receive_production/get_fqc_status 沿用
UPDATE inspection_results ir
SET source_id = sp.id
FROM stock_pickings sp
JOIN production_receipts pr ON pr.doc_number = sp.doc_number
WHERE ir.source_type = 5
  AND ir.source_id = pr.id
  AND sp.picking_type = 2;

-- 校验：行数应一致
-- SELECT
--   (SELECT COUNT(*) FROM production_receipts WHERE deleted_at IS NULL) AS pr_count,
--   (SELECT COUNT(*) FROM stock_pickings WHERE picking_type = 2 AND deleted_at IS NULL) AS sp_count;
