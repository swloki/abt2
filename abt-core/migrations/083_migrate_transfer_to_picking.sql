-- 数据迁移：inventory_transfers → stock_pickings（Issue #146 阶段 3）
-- 调拨（InternalTransfer）迁移到 stock_pickings，inventory_transfers / transfer_items 表保留只读历史。
--
-- 状态映射（值恰好相同，可直接 COPY）：
--   Draft=1 → Draft=1；InTransit=2 → Confirmed=2；Completed=3 → Done=3；Cancelled=4 → Cancelled=4
--
-- ⚠️ 生产执行前备份；项目无 runner，手动 psql -f

BEGIN;

-- 1. inventory_transfers → stock_pickings（picking_type=4 InternalTransfer）
INSERT INTO stock_pickings (
    doc_number, picking_type, status, source_type,
    from_warehouse_id, from_zone_id, from_bin_id,
    to_warehouse_id, to_zone_id, to_bin_id,
    operator_id, scheduled_date, remark, created_at, updated_at
)
SELECT
    it.doc_number,
    4,  -- PickingType::InternalTransfer
    it.status,  -- 值相同：1/2/3/4 → Draft/Confirmed/Done/Cancelled
    'none',
    it.from_warehouse_id, it.from_zone_id, it.from_bin_id,
    it.to_warehouse_id, it.to_zone_id, it.to_bin_id,
    it.operator_id,
    it.transfer_date,
    '',
    COALESCE(it.created_at, NOW()), COALESCE(it.created_at, NOW())
FROM inventory_transfers it
WHERE NOT EXISTS (
    SELECT 1 FROM stock_pickings sp
    WHERE sp.doc_number = it.doc_number AND sp.picking_type = 4
);

-- 2. transfer_items → stock_picking_items（通过 doc_number 关联新 picking）
INSERT INTO stock_picking_items (
    picking_id, product_id, qty_requested, qty_done, batch_no, remark, created_at
)
SELECT
    sp.id,
    ti.product_id,
    ti.quantity,
    0,  -- 调拨全量、无行级实绩；drawer 展示用 qty_requested
    ti.batch_no,
    '',
    COALESCE(it.created_at, NOW())
FROM transfer_items ti
JOIN inventory_transfers it ON it.id = ti.transfer_id
JOIN stock_pickings sp ON sp.doc_number = it.doc_number AND sp.picking_type = 4
WHERE NOT EXISTS (
    SELECT 1 FROM stock_picking_items spi
    WHERE spi.picking_id = sp.id AND spi.product_id = ti.product_id
      AND spi.qty_requested = ti.quantity
);

-- 校验：
-- SELECT
--   (SELECT COUNT(*) FROM inventory_transfers) AS trf,
--   (SELECT COUNT(*) FROM stock_pickings WHERE picking_type=4) AS pick,
--   (SELECT COUNT(*) FROM transfer_items) AS trf_items,
--   (SELECT COUNT(*) FROM stock_picking_items spi JOIN stock_pickings sp ON sp.id=spi.picking_id WHERE sp.picking_type=4) AS pick_items;

COMMIT;
