-- 099: DROP bom_routing_outputs（clean break 覆盖层退役）
--
-- 前置：098 已迁出数据（bom_operations + bom_step_prices）。部署 B 代码切源完成：
--   - production_batch load_operations_from_bom（读 bom_operations，不再读 bom_routing_outputs）
--   - bom try_build_labor_from_bom（切源）
--   - routing_detail overlay 整链删除 + routing update 覆盖护栏删除
--   - bom_routing_output/ 模块整体删除 + master_data/mod.rs 注销
-- 全仓 grep 确认无 bom_routing_outputs 读者（仅注释提及历史）。
--
-- 096 M2(b) 的 work_order_routings.product_id 快照修复历史结果已落库，不受 DROP 影响。
-- 本仓无 migration runner（手动 psql -f）。

DROP TABLE IF EXISTS bom_routing_outputs;
