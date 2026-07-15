-- scripts/backfill-wip-routing-output.sql
-- 修复历史批次：补录「改动前已报工、但半成品产出未入车间在制仓(WIP-SHOP)」的工序产出。
--
-- 背景：confirm_routing_step 在本次改动前不写库存（commit feat/mes-wip-routing-output），
-- 导致历史已报工的中间工序产出品（半成品）未入 WIP，后道齐套查 ATP<=0 → 死锁。
-- 本脚本对每个 (批次, 工序) 的累计合格报工量补一笔 RoutingOutput(13) 入 WIP-SHOP。
--
-- 幂等：以 source_type='work_report_backfill' + (product_id, source_id=routing_id) 判定，
--       可重复执行，已补录的跳过。
-- 局限：仅补产出侧（解死锁）。消耗侧（后道报工扣减上游半成品）历史未扣，WIP 余额可能虚高，
--       不影响流转；完工入库后可由 receive_production WIP 残留归零安全网处理。
--
-- 用法：psql "$DATABASE_URL" -f scripts/backfill-wip-routing-output.sql

DO $$
DECLARE
    wip_wh  BIGINT;
    wip_zone BIGINT;
    wip_bin  BIGINT;
    r RECORD;
    cnt INT;
BEGIN
    SELECT w.id, z.id, b.id INTO wip_wh, wip_zone, wip_bin
    FROM warehouses w
    JOIN zones z ON z.warehouse_id = w.id
    JOIN bins b ON b.zone_id = z.id
    WHERE w.code = 'WIP-SHOP' AND w.deleted_at IS NULL
    ORDER BY z.id, b.id LIMIT 1;
    IF wip_wh IS NULL THEN
        RAISE EXCEPTION 'WIP-SHOP warehouse not configured; run migration 105 first';
    END IF;

    FOR r IN
        SELECT
            wr_agg.batch_id,
            wor.id AS routing_id,
            wor.product_id AS out_pid,
            wr_agg.total_completed
        FROM (
            SELECT batch_id, routing_id, SUM(completed_qty) AS total_completed
            FROM work_reports GROUP BY batch_id, routing_id
        ) wr_agg
        JOIN work_order_routings wor ON wor.id = wr_agg.routing_id
        JOIN work_orders wo ON wo.id = wor.work_order_id
        WHERE wor.product_id IS NOT NULL
          AND wor.product_id <> wo.product_id   -- 半成品（≠工单成品 FG）
    LOOP
        SELECT COUNT(*) INTO cnt FROM inventory_transactions
        WHERE transaction_type = 13
          AND source_type = 'work_report_backfill'
          AND product_id = r.out_pid
          AND source_id = r.routing_id;
        IF cnt = 0 AND r.total_completed > 0 THEN
            INSERT INTO inventory_transactions (
                transaction_type, product_id, warehouse_id, zone_id, bin_id,
                quantity, source_type, source_id, remark, operator_id
            ) VALUES (
                13, r.out_pid, wip_wh, wip_zone, wip_bin,
                r.total_completed, 'work_report_backfill', r.routing_id,
                'backfill: routing output to WIP before wip-ledger fix', 1
            );
            INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, reserved_qty, available_qty)
            VALUES (r.out_pid, wip_wh, wip_zone, wip_bin, NULL, r.total_completed, 0, r.total_completed)
            ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, ''))
            DO UPDATE SET
                quantity = stock_ledger.quantity + EXCLUDED.quantity,
                available_qty = stock_ledger.available_qty + EXCLUDED.quantity;
            RAISE NOTICE 'Backfilled batch=% routing=% product=% qty=%',
                r.batch_id, r.routing_id, r.out_pid, r.total_completed;
        END IF;
    END LOOP;
END $$;

-- 验证：补录后的 WIP-SHOP 半成品余额
SELECT product_id, warehouse_id, quantity, available_qty
FROM stock_ledger
WHERE warehouse_id = (SELECT id FROM warehouses WHERE code='WIP-SHOP')
ORDER BY product_id;
