-- 放宽 production_plan_items 唯一约束：允许同一计划中同一产品来自不同销售订单
-- 根因：demand_handler 按 (product_id, sales_order_id) 聚合创建计划项，
-- 当多个销售订单需要同一产品时，原约束 UNIQUE(plan_id, product_id) 冲突。
-- 新约束 UNIQUE(plan_id, product_id, COALESCE(sales_order_id, 0)):
--   - 不同销售订单的同一产品：允许（修复 bug）
--   - 同一销售订单重复：仍然拦截
--   - 手动创建（sales_order_id = NULL）：COALESCE → 0，保持每产品唯一

ALTER TABLE production_plan_items
    DROP CONSTRAINT IF EXISTS production_plan_items_plan_id_product_id_key;

CREATE UNIQUE INDEX production_plan_items_plan_product_so_idx
    ON production_plan_items (plan_id, product_id, COALESCE(sales_order_id, 0));
