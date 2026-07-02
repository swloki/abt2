-- 084: shipping_requests → stock_pickings (OutgoingSales) 数据迁移
-- #146 阶段 4b：发货（outbound/shipping_requests）迁入 stock_pickings(OutgoingSales)
-- 物流字段（carrier/tracking_number/shipping_address）拼接存 stock_pickings.remark（用户决策）
-- status 映射：Draft→1, Confirmed→2, Picking→2（归并 Confirmed，历史拣货单视为待发）, Shipped→3(Done), Cancelled→4
-- 幂等：NOT EXISTS doc_number 避免重复；shipping_requests 表保留归档（阶段 6 DROP）

INSERT INTO stock_pickings
    (doc_number, picking_type, status, source_type, source_id, partner_id,
     from_warehouse_id, scheduled_date, done_at, remark, operator_id, created_at, updated_at, deleted_at)
SELECT
    sr.doc_number,
    3,  -- PickingType::OutgoingSales
    CASE sr.status
        WHEN 1 THEN 1  -- Draft
        WHEN 2 THEN 2  -- Confirmed
        WHEN 3 THEN 2  -- Picking → Confirmed（归并，拣货已移除）
        WHEN 4 THEN 3  -- Shipped → Done
        WHEN 5 THEN 4  -- Cancelled
    END AS status,
    'sales_order',
    sr.order_id,
    sr.customer_id,
    NULL,  -- from_warehouse_id（销售申请不指定仓，direct_ship 选仓时填）
    sr.expected_ship_date,
    CASE WHEN sr.status = 4 THEN sr.updated_at ELSE NULL END,  -- done_at（Shipped 用 updated_at 兜底）
    CONCAT_WS(' | ',
        NULLIF(sr.carrier, ''),
        NULLIF(sr.tracking_number, ''),
        NULLIF(sr.shipping_address, ''),
        NULLIF(sr.remark, '')
    ),
    sr.operator_id,
    sr.created_at,
    sr.updated_at,
    sr.deleted_at
FROM shipping_requests sr
WHERE NOT EXISTS (
    SELECT 1 FROM stock_pickings sp
    WHERE sp.picking_type = 3 AND sp.doc_number = sr.doc_number
);

-- shipping_request_items → stock_picking_items
INSERT INTO stock_picking_items
    (picking_id, product_id, qty_requested, qty_done, source_item_id, remark, created_at)
SELECT
    sp.id,
    sri.product_id,
    sri.requested_qty,
    sri.shipped_qty,  -- qty_done（Shipped 行 shipped_qty = requested_qty）
    sri.order_item_id,  -- source_item_id（关联销售订单行）
    COALESCE(NULLIF(sri.description, ''), ''),
    sri.created_at
FROM shipping_request_items sri
JOIN shipping_requests sr ON sr.id = sri.shipping_request_id
JOIN stock_pickings sp ON sp.picking_type = 3 AND sp.doc_number = sr.doc_number
WHERE NOT EXISTS (
    SELECT 1 FROM stock_picking_items spi
    WHERE spi.picking_id = sp.id AND spi.source_item_id = sri.order_item_id
);

-- 校验：行数应一致（shipping_requests vs stock_pickings where picking_type=3）
-- SELECT
--   (SELECT COUNT(*) FROM shipping_requests WHERE deleted_at IS NULL) AS sr_count,
--   (SELECT COUNT(*) FROM stock_pickings WHERE picking_type = 3 AND deleted_at IS NULL) AS sp_count;
