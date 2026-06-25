-- 074: 销售订单「已申请发货」(ShippingRequested) 状态 + 发货/拣货明细仓库可空
--
-- 语义：销售在订单详情页一键「申请发货」后，订单从 Confirmed/ReadyToShip/PartiallyShipped
-- 推进到 ShippingRequested（已提交发货申请，待仓库拣货发货）。填补 SalesOrderStatus 枚举值 8。
-- 推导逻辑见 sales_order/implt.rs::recalc_header_status（有活跃发货单 Confirmed/Picking 且未全 Shipped → ShippingRequested）。
--
-- 同时：发货明细/拣货明细的 warehouse_id 改可空——销售申请时不指定仓库（预留本就跨仓库 ATP），
-- 仓库拣货时手选仓库库位（pick_list_items.warehouse_id/bin_id 录入），ship 扣库存用拣货录入的仓库。
--
-- 幂等：UNIQUE(entity_type, from_state, to_state) 已存在，ON CONFLICT DO NOTHING 保证可重复执行。

BEGIN;

INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('SalesOrderStatus', 'Confirmed',        'ShippingRequested', NULL, 20),
    ('SalesOrderStatus', 'ReadyToShip',      'ShippingRequested', NULL, 21),
    ('SalesOrderStatus', 'PartiallyShipped', 'ShippingRequested', NULL, 22),
    ('SalesOrderStatus', 'ShippingRequested', 'PartiallyShipped', NULL, 23),
    ('SalesOrderStatus', 'ShippingRequested', 'Shipped',          NULL, 24),
    ('SalesOrderStatus', 'ShippingRequested', 'Cancelled',        NULL, 25),
    ('SalesOrderStatus', 'ShippingRequested', 'Completed',        NULL, 26)
ON CONFLICT DO NOTHING;

-- 发货明细仓库可空（NULL = 申请阶段未选仓库，仓库拣货时定）
ALTER TABLE shipping_request_items ALTER COLUMN warehouse_id DROP NOT NULL;

-- 拣货明细仓库可空（NULL = 待拣货时手选，拣货录入后回填）
ALTER TABLE pick_list_items ALTER COLUMN warehouse_id DROP NOT NULL;

COMMIT;
