-- 移除销售订单 Completed 终态的状态机转换（Issue #220）
-- complete() 方法及 complete_order handler 为死代码（无 UI 入口、无业务挂钩），
-- 财务立账（AR + COGS）在发货时由 ShipmentShippedHandler 完成，不依赖 Completed。
-- DB 无 status=6 订单，可直接删除两条状态机转换定义。
DELETE FROM state_transition_defs
WHERE entity_type = 'SalesOrderStatus' AND to_state = 'Completed';
