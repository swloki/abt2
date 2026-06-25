-- backfill_ready_to_ship.sql
-- 将现存「Confirmed 且全行 Allocated(2) 且从未发货」的销售订单推进到 ReadyToShip(3)
--
-- 背景：这些订单在本次改动之前 confirm（旧 confirm 不带 recalc 推进），滞留于 Confirmed。
--   重启服务加载新代码后，仅"新 confirm"的订单会自动进入待发货；存量需本脚本补齐。
--
-- ⚠ 执行时机：必须在「服务重启加载新代码之后」执行。
--    旧二进制的 SalesOrderStatus::from_i16 不识别 3，若在重启前执行，旧服务读取这些订单会 500。
--
-- 幂等：targets 仅匹配 status=2，已推进到 3 的不会重复处理，可多次执行。
-- 前置：state_transition_defs 已含 Confirmed→ReadyToShip（见 073_sales_order_ready_to_ship.sql）。

BEGIN;

-- 预览影响范围（可单独执行此 SELECT 核对）
-- SELECT so.id, so.doc_number FROM sales_orders so
-- JOIN sales_order_items soi ON soi.order_id = so.id
-- WHERE so.status = 2 AND so.deleted_at IS NULL
-- GROUP BY so.id
-- HAVING COUNT(*) FILTER (WHERE soi.shipped_qty > 0) = 0
--    AND COUNT(*) = COUNT(*) FILTER (WHERE soi.line_status = 2);

-- 1) 写状态机日志（from=Confirmed → to=ReadyToShip）
INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, remark)
SELECT
    'SalesOrderStatus',
    so.id,
    'Confirmed',
    'ReadyToShip',
    (SELECT id FROM state_transition_defs
       WHERE entity_type = 'SalesOrderStatus' AND from_state = 'Confirmed' AND to_state = 'ReadyToShip'),
    'backfill: 全行 Allocated、未发货'
FROM sales_orders so
JOIN sales_order_items soi ON soi.order_id = so.id
WHERE so.status = 2 AND so.deleted_at IS NULL
GROUP BY so.id
HAVING COUNT(*) FILTER (WHERE soi.shipped_qty > 0) = 0
   AND COUNT(*) = COUNT(*) FILTER (WHERE soi.line_status = 2);

-- 2) 更新订单头状态
UPDATE sales_orders
SET status = 3, updated_at = now()
WHERE id IN (
    SELECT so.id
    FROM sales_orders so
    JOIN sales_order_items soi ON soi.order_id = so.id
    WHERE so.status = 2 AND so.deleted_at IS NULL
    GROUP BY so.id
    HAVING COUNT(*) FILTER (WHERE soi.shipped_qty > 0) = 0
       AND COUNT(*) = COUNT(*) FILTER (WHERE soi.line_status = 2)
);

COMMIT;
