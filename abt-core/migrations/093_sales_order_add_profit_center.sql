-- ============================================================================
-- 093: sales_orders 加 profit_center_id — 利润中心归集来源
-- 接单时选定利润中心，向下传递：发货/对账/工单归集时反查此字段填 cost_entries.profit_center
-- ============================================================================

BEGIN;

ALTER TABLE sales_orders ADD COLUMN IF NOT EXISTS profit_center_id BIGINT REFERENCES profit_centers(id);

CREATE INDEX IF NOT EXISTS idx_sales_orders_profit_center ON sales_orders (profit_center_id) WHERE deleted_at IS NULL;

COMMIT;
