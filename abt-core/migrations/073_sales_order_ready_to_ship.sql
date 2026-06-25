-- 073: 销售订单新增「待发货」(ReadyToShip) 状态转换
--
-- 语义：库存已补足（全行 line_status = Allocated）、从未发货，
-- 可发货但尚未发货。填补 SalesOrderStatus 枚举值 3（Draft=1 / Confirmed=2 / 〔3〕 / PartiallyShipped=4）。
-- 推导逻辑见 sales_order/implt.rs::calc_header_status。
--
-- 幂等：UNIQUE(entity_type, from_state, to_state) 已存在，ON CONFLICT DO NOTHING 保证可重复执行。

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('SalesOrderStatus', 'Confirmed',   'ReadyToShip',      NULL, 10),
    ('SalesOrderStatus', 'ReadyToShip', 'PartiallyShipped', NULL, 11),
    ('SalesOrderStatus', 'ReadyToShip', 'Shipped',          NULL, 12),
    ('SalesOrderStatus', 'ReadyToShip', 'Cancelled',        NULL, 13)
ON CONFLICT DO NOTHING;
