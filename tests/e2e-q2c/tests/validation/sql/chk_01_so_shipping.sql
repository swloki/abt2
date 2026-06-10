-- CHK-01: SO 明细数量 >= 发货数量
-- 验证: 每个订单行已发货数量不超过订单数量
SELECT so.id AS order_id, so.doc_number,
       soi.id AS item_id, p.product_code,
       soi.quantity AS ordered_qty,
       COALESCE(sri.shipped_qty, 0) AS shipped_qty,
       soi.quantity - COALESCE(sri.shipped_qty, 0) AS remaining
FROM sales_orders so
JOIN sales_order_items soi ON soi.order_id = so.id
JOIN products p ON soi.product_id = p.product_id
LEFT JOIN (
    SELECT sri.order_item_id, SUM(sri.shipped_qty) AS shipped_qty
    FROM shipping_request_items sri
    JOIN shipping_requests sr ON sri.shipping_request_id = sr.id AND sr.deleted_at IS NULL AND sr.status = 4
    WHERE sri.deleted_at IS NULL
    GROUP BY sri.order_item_id
) sri ON sri.order_item_id = soi.id
WHERE so.deleted_at IS NULL
  AND so.doc_number LIKE 'SO-%'
  AND COALESCE(sri.shipped_qty, 0) > soi.quantity;
-- 预期: 0 行返回（无超发）
