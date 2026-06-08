-- FMS Cost Analysis 测试数据
-- 关联到真实 products, work_orders, sales_orders, departments

-- 清除旧 cost_entries
DELETE FROM cost_entries;

-- ============================================================
-- 1. 产品成本（entity_type=1, entity_id=product_id）
--    CostType: 1=材料, 2=人工, 3=制造费用
-- ============================================================

-- 产品 565: 3010134033  2835/冷白0.5W/RA70-单晶-3C02
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(1, 565, 1, 186000.00, 0, 1, 1, '2026-06', 15, 1),
(1, 565, 2,  42000.00, 0, 1, 1, '2026-06', 15, 1),
(1, 565, 3,  18000.00, 0, 1, 1, '2026-06', 15, 1),
(1, 565, 1, 165000.00, 0, 1, 1, '2026-05', 15, 1),
(1, 565, 2,  38000.00, 0, 1, 1, '2026-05', 15, 1),
(1, 565, 3,  15000.00, 0, 1, 1, '2026-05', 15, 1);

-- 产品 566: x1739581537  灯珠/2835/正白0.3W/RA70-单晶-2C00
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(1, 566, 1, 320000.00, 0, 1, 2, '2026-06', 15, 2),
(1, 566, 2,  56000.00, 0, 1, 2, '2026-06', 15, 2),
(1, 566, 3,  24000.00, 0, 1, 2, '2026-06', 15, 2);

-- 产品 567: 3010135078  2835/正白0.3W/RA70-单晶-2C00(HSG)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(1, 567, 1, 125000.00, 0, 1, 3, '2026-06', 15, 3),
(1, 567, 2,  37500.00, 0, 1, 3, '2026-06', 15, 3),
(1, 567, 3,  12500.00, 0, 1, 3, '2026-06', 15, 3);

-- 产品 568: 3090354936  AC插座/DB-8(AC-019A)/白色
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(1, 568, 1, 90000.00, 0, 1, 4, '2026-06', 15, 4),
(1, 568, 2, 24000.00, 0, 1, 4, '2026-06', 15, 4),
(1, 568, 3,  6000.00, 0, 1, 4, '2026-06', 15, 4);

-- ============================================================
-- 2. 工单成本（entity_type=2, entity_id=work_order_id）
-- ============================================================

-- WO id=1 (WO-2026-06-000001, 产品565)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(2, 1, 1, 82500.00, 0, 1, 1, '2026-06', 15, 1),
(2, 1, 2, 18600.00, 0, 1, 1, '2026-06', 15, 1),
(2, 1, 3,  4200.00, 0, 1, 1, '2026-06', 15, 1);

-- WO id=2 (WO-2026-06-000002, 产品566)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(2, 2, 1, 128000.00, 0, 1, 2, '2026-06', 15, 2),
(2, 2, 2,  22400.00, 0, 1, 2, '2026-06', 15, 2),
(2, 2, 3,   8500.00, 0, 1, 2, '2026-06', 15, 2);

-- WO id=3 (WO-2026-06-000003, 产品565)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(2, 3, 1, 52000.00, 0, 1, 1, '2026-06', 15, 3),
(2, 3, 2, 15600.00, 0, 1, 1, '2026-06', 15, 3),
(2, 3, 3,  3800.00, 0, 1, 1, '2026-06', 15, 3);

-- WO id=5 (WO-2026-06-000005, 产品566)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(2, 5, 1, 36000.00, 0, 1, 2, '2026-06', 15, 5),
(2, 5, 2,  9600.00, 0, 1, 2, '2026-06', 15, 5),
(2, 5, 3,  2400.00, 0, 1, 2, '2026-06', 15, 5);

-- ============================================================
-- 3. 利润中心 P&L（通过 profit_center 列 = department_id）
--    profit_center 1=生产部, 2=仓库管理, 3=采购部, 4=品质部
-- ============================================================

-- 利润中心1: 生产部
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(4, 1, 1, 500000.00, 0, 1, 1, '2026-06', 15, 1),
(4, 1, 2, 128000.00, 0, 1, 1, '2026-06', 15, 1),
(4, 1, 3,  42800.00, 0, 1, 1, '2026-06', 15, 1),
(4, 1, 1,  28600.00, 0, 1, 1, '2026-06', 15, 1),  -- 管理费用用 cost_type=1 的 debit
(4, 1, 4, 850000.00, 850000.00, 1, 1, '2026-06', 16, 1);  -- 收入: credit

-- 利润中心2: 仓库管理
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(4, 2, 1, 374900.00, 0, 2, 2, '2026-06', 15, 2),
(4, 2, 2,  93700.00, 0, 2, 2, '2026-06', 15, 2),
(4, 2, 3,  31200.00, 0, 2, 2, '2026-06', 15, 2),
(4, 2, 1,  21800.00, 0, 2, 2, '2026-06', 15, 2),
(4, 2, 4, 624000.00, 624000.00, 2, 2, '2026-06', 16, 2);

-- 利润中心3: 采购部
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(4, 3, 1, 268200.00, 0, 3, 3, '2026-06', 15, 3),
(4, 3, 2,  61900.00, 0, 3, 3, '2026-06', 15, 3),
(4, 3, 3,  20600.00, 0, 3, 3, '2026-06', 15, 3),
(4, 3, 1,  18400.00, 0, 3, 3, '2026-06', 15, 3),
(4, 3, 4, 412000.00, 412000.00, 3, 3, '2026-06', 16, 3);

-- ============================================================
-- 4. 销售订单毛利（entity_type=3, entity_id=sales_order_id）
-- ============================================================

-- SO id=22 (SO-TEST-0001, customer=深圳光电科技, amount=782.50)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(3, 22, 1, 520.00, 0, NULL, 1, '2026-06', 15, 22),
(3, 22, 2, 120.00, 0, NULL, 1, '2026-06', 15, 22),
(3, 22, 3,  45.00, 0, NULL, 1, '2026-06', 15, 22);

-- SO id=5 (SO-2026-05-00005, customer=wew, amount=11.00)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(3, 5, 1, 5.50, 0, NULL, 2, '2026-05', 15, 5),
(3, 5, 2, 2.20, 0, NULL, 2, '2026-05', 15, 5),
(3, 5, 3, 0.80, 0, NULL, 2, '2026-05', 15, 5);

-- SO id=19 (SO-2026-05-000013, customer=wew, amount=33.00)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(3, 19, 1, 18.00, 0, NULL, 1, '2026-06', 15, 19),
(3, 19, 2,  6.50, 0, NULL, 1, '2026-06', 15, 19),
(3, 19, 3,  2.80, 0, NULL, 1, '2026-06', 15, 19);

-- SO id=13 (SO-2026-05-00010, customer=sdsd, amount=11.00)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(3, 13, 1, 6.00, 0, NULL, 3, '2026-06', 15, 13),
(3, 13, 2, 2.50, 0, NULL, 3, '2026-06', 15, 13),
(3, 13, 3, 1.20, 0, NULL, 3, '2026-06', 15, 13);

-- SO id=12 (SO-2026-05-00009, customer=sdsd, amount=0.00)
INSERT INTO cost_entries (entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id) VALUES
(3, 12, 1, 3.20, 0, NULL, 3, '2026-05', 15, 12),
(3, 12, 2, 1.50, 0, NULL, 3, '2026-05', 15, 12);
