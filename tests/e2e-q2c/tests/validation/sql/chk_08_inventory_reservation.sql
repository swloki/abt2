-- CHK-08: 库存预留一致性
-- 验证: shipping_requests 中未发货的预留库存合计
SELECT sr.id, sr.doc_number, sr.status,
       SUM(sri.requested_qty) - SUM(COALESCE(sri.shipped_qty, 0)) AS pending_qty
FROM shipping_requests sr
JOIN shipping_request_items sri ON sri.shipping_request_id = sr.id
WHERE sr.deleted_at IS NULL AND sr.status IN (1, 2, 3)  -- Draft/Confirmed/Picking
  AND sri.deleted_at IS NULL
GROUP BY sr.id, sr.doc_number, sr.status
HAVING SUM(sri.requested_qty) - SUM(COALESCE(sri.shipped_qty, 0)) < 0;
-- 预期: 0 行返回（预留不超发）
