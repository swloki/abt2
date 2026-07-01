-- 078: ShippingStatus 加 Confirmed → Shipped（直接发，跳过 Picking）
-- 配合 outbound::direct_ship：未拣（Confirmed）单选仓直接发货，拣货设为可选
-- （参考 Odoo 默认 ship_only / ERPNext 直接建 Delivery Note / OFBiz quick ship）。
-- 注：ShippingStatus 现有转换（Draft→Confirmed→Picking→Shipped、Cancelled 等）历史性不在
-- migration 文件（DB 手动插入），本文件只补 Confirmed→Shipped 这一条。
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, guard_condition, side_effects)
VALUES ('ShippingStatus', 'Confirmed', 'Shipped', NULL, NULL, '[]')
ON CONFLICT DO NOTHING;
