-- CHK-03: PO 与收货一致性
-- 验证: 收货数量 <= PO 数量
SELECT po.id AS po_id, po.doc_number,
       poi.id AS item_id, p.product_code,
       poi.quantity AS ordered_qty,
       COALESCE(sri.declared_qty, 0) AS received_qty,
       COALESCE(sri.declared_qty, 0) - poi.quantity AS over_receipt
FROM purchase_orders po
JOIN purchase_order_items poi ON poi.order_id = po.id
JOIN products p ON poi.product_id = p.product_id
LEFT JOIN (
    SELECT ani.product_id, an.purchase_order_id, SUM(ani.declared_qty) AS declared_qty
    FROM arrival_notice_items ani
    JOIN arrival_notices an ON ani.notice_id = an.id AND an.deleted_at IS NULL
    WHERE 1=1
    GROUP BY ani.product_id, an.purchase_order_id
) sri ON sri.product_id = poi.product_id AND sri.purchase_order_id = po.id
WHERE po.deleted_at IS NULL
  AND COALESCE(sri.declared_qty, 0) > poi.quantity;
-- 预期: 0 行返回（无超收）
