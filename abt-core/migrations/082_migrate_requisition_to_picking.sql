-- 数据迁移：material_requisitions → stock_pickings（Issue #146 阶段 2）
-- 领料单（InternalIssue）直接迁移到 stock_pickings，material_requisition 模块已删除。
-- 迁移后 material_requisitions / material_requisition_items 表保留（只读历史，便于核对），不 DROP。
--
-- 状态映射（requisition → picking）：
--   Draft=1 → Draft=1；Confirmed=2 → Confirmed=2；Issued=3 → Done=3；
--   Cancelled=4 → Cancelled=4；PartiallyIssued=5 → Confirmed=2（行级 qty_done 表达部分量）
--
-- ⚠️ 生产执行前务必备份；执行后用校验 SQL 核对 row count。
-- 项目无 migration runner，手动：psql -f 082_migrate_requisition_to_picking.sql

BEGIN;

-- 1. material_requisitions → stock_pickings（picking_type=5 InternalIssue）
--    doc_number 保留原 MR 号（便于追溯；与新建 picking 的 LL 前缀不冲突）
INSERT INTO stock_pickings (
    doc_number, picking_type, status, source_type, source_id,
    from_warehouse_id, operator_id, scheduled_date, work_order_id,
    remark, created_at, updated_at, deleted_at
)
SELECT
    mr.doc_number,
    5,  -- PickingType::InternalIssue
    CASE mr.status
        WHEN 1 THEN 1  -- Draft
        WHEN 2 THEN 2  -- Confirmed
        WHEN 3 THEN 3  -- Issued → Done
        WHEN 4 THEN 4  -- Cancelled
        WHEN 5 THEN 2  -- PartiallyIssued → Confirmed
    END,
    CASE WHEN mr.work_order_id > 0 THEN 'work_order' ELSE 'none' END,
    CASE WHEN mr.work_order_id > 0 THEN mr.work_order_id ELSE NULL END,
    mr.warehouse_id,
    mr.operator_id,
    mr.requisition_date,
    CASE WHEN mr.work_order_id > 0 THEN mr.work_order_id ELSE NULL END,
    '',
    mr.created_at, mr.updated_at, mr.deleted_at
FROM material_requisitions mr
WHERE NOT EXISTS (
    SELECT 1 FROM stock_pickings sp
    WHERE sp.doc_number = mr.doc_number AND sp.picking_type = 5
);

-- 2. material_requisition_items → stock_picking_items（通过 doc_number 关联到新 picking）
INSERT INTO stock_picking_items (
    picking_id, product_id, qty_requested, qty_done,
    from_bin_id, operation_id, batch_id, remark, created_at
)
SELECT
    sp.id,
    mri.product_id,
    mri.requested_qty,
    mri.issued_qty,
    mri.bin_id,
    mri.operation_id,
    mri.batch_id,
    '',
    mr.created_at
FROM material_requisition_items mri
JOIN material_requisitions mr ON mr.id = mri.requisition_id
JOIN stock_pickings sp ON sp.doc_number = mr.doc_number AND sp.picking_type = 5
WHERE NOT EXISTS (
    SELECT 1 FROM stock_picking_items spi
    WHERE spi.picking_id = sp.id AND spi.product_id = mri.product_id
      AND spi.qty_requested = mri.requested_qty
      AND COALESCE(spi.operation_id, -1) = COALESCE(mri.operation_id, -1)
      AND COALESCE(spi.batch_id, -1) = COALESCE(mri.batch_id, -1)
);

-- 3. document_links：source_type=MaterialRequisition(17) 的行，source_id 从 requisition.id 更新为 picking.id
--    （picking 借用 DocumentType::MaterialRequisition variant 做 link 类型，work_order cancel 反查逻辑不变；
--     仅 source_id 指向新 picking，使 cancel 能命中 picking.cancel）
UPDATE document_links dl
SET source_id = sp.id
FROM stock_pickings sp
JOIN material_requisitions mr ON mr.doc_number = sp.doc_number
WHERE dl.source_type = 17  -- DocumentType::MaterialRequisition
  AND dl.source_id = mr.id
  AND sp.picking_type = 5;

-- 校验：迁移后 picking 数 = 原 requisition 数（排除已存在）
-- SELECT
--   (SELECT COUNT(*) FROM material_requisitions) AS req_total,
--   (SELECT COUNT(*) FROM stock_pickings WHERE picking_type = 5) AS picking_total,
--   (SELECT COUNT(*) FROM material_requisition_items) AS req_items,
--   (SELECT COUNT(*) FROM stock_picking_items spi JOIN stock_pickings sp ON sp.id = spi.picking_id WHERE sp.picking_type = 5) AS picking_items;

COMMIT;
