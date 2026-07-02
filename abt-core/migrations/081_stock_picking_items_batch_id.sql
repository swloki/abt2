-- stock_picking_items 补 batch_id（Issue #146 阶段 2，对齐 MES 工序领料批次）
-- 阶段 1 建表时已含 operation_id；batch_id 对应 material_req_items.batch_id（MES 工序级领料的生产批次）。
-- 领料单（InternalIssue）直接迁移到 stock_pickings 后，picking_items 需承载 batch_id。

ALTER TABLE stock_picking_items ADD COLUMN IF NOT EXISTS batch_id BIGINT;
CREATE INDEX IF NOT EXISTS idx_stock_picking_items_batch
    ON stock_picking_items(batch_id) WHERE batch_id IS NOT NULL;
