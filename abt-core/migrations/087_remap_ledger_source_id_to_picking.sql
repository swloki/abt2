-- 087: 历史台账 source_id 重映射（shipping_request.id → stock_pickings.id via doc_number）
-- #146 阶段 6a：ar_ap/reconciliation SQL 切 stock_pickings 后，历史台账 source_type=3 (ShippingRequest)
-- 的 source_id 仍指向旧 shipping_request.id，需经 doc_number 映射到 stock_pickings.id
-- （084 迁移保留 doc_number；JOIN shipping_requests 仅用于建立 id 映射，阶段 6b DROP 前必须完成）

-- ar_ap_ledger: source_type=3 (ShippingRequest) 应收台账
UPDATE ar_ap_ledger l
SET source_id = sp.id
FROM stock_pickings sp
JOIN shipping_requests sr ON sr.doc_number = sp.doc_number
WHERE l.source_type = 3
  AND l.source_id = sr.id
  AND sp.picking_type = 3;

-- cost_entries: source_type=3 (ShippingRequest) COGS 分录
UPDATE cost_entries ce
SET source_id = sp.id
FROM stock_pickings sp
JOIN shipping_requests sr ON sr.doc_number = sp.doc_number
WHERE ce.source_type = 3
  AND ce.source_id = sr.id
  AND sp.picking_type = 3;

-- 校验：重映射后 source_type=3 的 source_id 应全部匹配 stock_pickings
-- SELECT
--   (SELECT COUNT(*) FROM ar_ap_ledger WHERE source_type=3) AS ar_total,
--   (SELECT COUNT(*) FROM ar_ap_ledger l WHERE l.source_type=3 AND EXISTS(SELECT 1 FROM stock_pickings sp WHERE sp.id=l.source_id AND sp.picking_type=3)) AS ar_match_pick,
--   (SELECT COUNT(*) FROM cost_entries WHERE source_type=3) AS ce_total,
--   (SELECT COUNT(*) FROM cost_entries ce WHERE ce.source_type=3 AND EXISTS(SELECT 1 FROM stock_pickings sp WHERE sp.id=ce.source_id AND sp.picking_type=3)) AS ce_match_pick;
