-- 修复断链采购单（库存入库页 source_type=purchase 绕过来料通知导致 received_qty=0）
-- 根因：create_stock_in 历史版本直接 record() 写库存，未回写 PO/立台账。
-- 本脚本为每个断链 PO 补一条来料通知(Accepted) + 明细 + 单据关联 + 回写 received_qty + 立应付台账。
-- received_qty = LEAST(实际入库量, 订单量)（超收按订单量截断，不多付应付）。
-- 单事务 + ON_ERROR_STOP，失败整体回滚。不动库存（inventory_transactions/stock_ledger）。
\set ON_ERROR_STOP on
SET client_encoding TO 'UTF8';
BEGIN;

-- 聚合断链 PO 的入库流水：status=2 且有 source_type=purchase_order 的入库
CREATE TEMP TABLE _broken_stock AS
SELECT it.source_id AS po_id, it.product_id,
       SUM(it.quantity) AS stockin_qty,
       (array_agg(it.warehouse_id ORDER BY it.created_at))[1] AS wh_id
FROM inventory_transactions it
JOIN purchase_orders po ON po.id = it.source_id
WHERE it.source_type = 'purchase_order' AND po.status = 2
  AND po.deleted_at IS NULL AND it.quantity > 0
GROUP BY it.source_id, it.product_id;

DO $$
DECLARE
    po_rec RECORD;
    item_rec RECORD;
    new_an_id BIGINT;
    ap_amount NUMERIC(18,6);
BEGIN
    FOR po_rec IN
        SELECT DISTINCT bs.po_id, po.supplier_id, po.operator_id,
               (SELECT wh_id FROM _broken_stock WHERE po_id = po.id LIMIT 1) AS wh_id
        FROM _broken_stock bs
        JOIN purchase_orders po ON po.id = bs.po_id
    LOOP
        -- 来料通知（Accepted=4）
        INSERT INTO arrival_notices (doc_number, purchase_order_id, supplier_id, arrival_date, status, warehouse_id, operator_id)
        VALUES ('AN-FIX-' || po_rec.po_id, po_rec.po_id, po_rec.supplier_id, '2026-06-25', 4, po_rec.wh_id, po_rec.operator_id)
        RETURNING id INTO new_an_id;

        ap_amount := 0;
        FOR item_rec IN
            SELECT poi.id AS poi_id, poi.product_id, poi.quantity, poi.unit_price, bs.stockin_qty,
                   LEAST(bs.stockin_qty, poi.quantity) AS recv
            FROM purchase_order_items poi
            JOIN _broken_stock bs ON bs.po_id = poi.order_id AND bs.product_id = poi.product_id
            WHERE poi.order_id = po_rec.po_id
        LOOP
            INSERT INTO arrival_notice_items (notice_id, order_item_id, product_id, declared_qty, received_qty, accepted_qty)
            VALUES (new_an_id, item_rec.poi_id, item_rec.product_id, item_rec.recv, item_rec.recv, item_rec.recv);
            UPDATE purchase_order_items SET received_qty = item_rec.recv WHERE id = item_rec.poi_id;
            ap_amount := ap_amount + item_rec.recv * item_rec.unit_price;
        END LOOP;

        -- 单据关联 AN(16=ArrivalNotice) → PO(7=PurchaseOrder) Fulfills(6)
        INSERT INTO document_links (source_type, source_id, target_type, target_id, link_type, path, depth, created_by)
        VALUES (16, new_an_id, 7, po_rec.po_id, 6, 'AN.' || new_an_id || '.PO.' || po_rec.po_id, 1, po_rec.operator_id);

        -- PO 状态 → Received(4)
        UPDATE purchase_orders SET status = 4, updated_at = now() WHERE id = po_rec.po_id;

        -- 应付台账（Credit，金额=Σ截断后入库量×单价；幂等兜底）
        INSERT INTO ar_ap_ledger (party_type, party_id, source_type, source_id, source_doc_no, direction, amount, currency, exchange_rate, transaction_date, period, description, operator_id)
        VALUES (2, po_rec.supplier_id, 16, new_an_id, 'AN-' || new_an_id, 2, ap_amount, 'CNY', 1, '2026-06-25', '2026-06',
                '采购入库(历史修复) AN-' || new_an_id, po_rec.operator_id)
        ON CONFLICT DO NOTHING;

        RAISE NOTICE '修复 PO#% -> AN-FIX-% (id=%) 应付=%', po_rec.po_id, po_rec.po_id, new_an_id, ap_amount;
    END LOOP;
END $$;

DROP TABLE _broken_stock;
COMMIT;

-- ── 验证 ──
SELECT '断链PO剩余(应0)' AS chk, COUNT(*) AS n
FROM purchase_orders po
WHERE po.status = 2 AND po.deleted_at IS NULL
  AND EXISTS (SELECT 1 FROM inventory_transactions it WHERE it.source_type='purchase_order' AND it.source_id=po.id AND it.quantity>0);

SELECT 'AN-FIX来料通知(应17)' AS chk, COUNT(*) AS n FROM arrival_notices WHERE doc_number LIKE 'AN-FIX-%';

SELECT '新增应付台账(应17)' AS chk, COUNT(*) AS n FROM ar_ap_ledger WHERE description LIKE '%历史修复%';

SELECT 'AN-FIX单据关联(应17)' AS chk, COUNT(*) AS n
FROM document_links
WHERE source_type=16 AND target_type=7 AND link_type=6
  AND source_id IN (SELECT id FROM arrival_notices WHERE doc_number LIKE 'AN-FIX-%');
