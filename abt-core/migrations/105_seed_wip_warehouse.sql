-- 105: 车间在制虚拟仓（工序产出半成品「车间在制」账面流转，解 MES 工序流转死锁）
--
-- 背景：多工序工单（注塑→组装→包装），前道报工产出的半成品原先不入库，
-- 导致后道齐套检查 ATP=0 → 欠料 → 领料置灰 → 死锁。
-- 解法：报工事务（confirm_routing_step）为半成品记两笔账——
--   产出 +qty（TransactionType::RoutingOutput=13）入此仓；
--   后道消耗 -qty（MaterialIssue=4）从此仓扣。
-- is_virtual=true：非物理仓，无搬运动作，纯账面流转（仿委外 VirtualOutsource）。
-- WarehouseType::SemiFinished=3。
-- 单 bin 承载多半成品 SKU：record() 传 zone/bin=None 时 resolve_default_bin 兜底落首个 bin。

INSERT INTO warehouses (code, name, warehouse_type, status, is_virtual, remark, operator_id)
VALUES ('WIP-SHOP', '车间在制库', 3, 1, true, '工序产出半成品在制流转虚拟仓（报工自动产出/消耗，非物理仓）', 1)
ON CONFLICT (code) DO NOTHING;

INSERT INTO zones (warehouse_id, code, name, zone_type)
SELECT id, 'WIP-ZONE', '在制区', 2 FROM warehouses WHERE code = 'WIP-SHOP'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name)
SELECT z.id, 'WIP-BIN', '在制储位'
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WIP-SHOP'
ON CONFLICT (zone_id, code) DO NOTHING;
