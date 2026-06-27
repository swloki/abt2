-- 076: 扁平化——废弃生产订单(PP)层，删除 production_plans / production_plan_items 表
-- 对应 Issue #116。代码层 production_plan 模块已删，WO 状态机自洽（Draft→Released→InProduction→Closed）。
-- 生产链路扁平化为：销售订单 → 需求池 → 工单(WO) → 生产批次（对标 Odoo/OFBiz）。
-- ⚠ 破坏性迁移（删表不可逆），手动 psql -f 执行。

BEGIN;

-- 1. work_orders.plan_item_id 去外键约束 + 索引
--    （列保留为 nullable 历史字段，扁平化后新工单不再写入；旧值悬空无害）
ALTER TABLE work_orders DROP CONSTRAINT IF EXISTS work_orders_plan_item_id_fkey;
DROP INDEX IF EXISTS idx_work_orders_plan_item;

-- 2. demands.target_doc 的 PP 历史置空（target_doc_type=12=ProductionPlan）
--    新流程 target_doc_type=10=WorkOrder（需求直达 Draft 工单）
UPDATE demands SET target_doc_type = NULL, target_doc_id = NULL
WHERE target_doc_type = 12;

-- 3. 清理 ProductionPlan 状态机定义（WorkOrder/ProductionBatch/ProductionReceipt 保留）
DELETE FROM state_transition_defs WHERE entity_type = 'ProductionPlan';
DELETE FROM state_definitions    WHERE entity_type = 'ProductionPlan';

-- 4. 删 PP 表（production_plan_items 先删，此时 work_orders.plan_item_id 已无 FK）
DROP TABLE IF EXISTS production_plan_items;
DROP TABLE IF EXISTS production_plans;

COMMIT;
