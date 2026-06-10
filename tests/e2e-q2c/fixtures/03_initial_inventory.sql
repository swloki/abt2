-- ============================================================================
-- Q2C E2E 测试 — 初始库存
-- 预置原材料库存到 WH-RAW，成品仓 WH-FG 初始为空
-- 脚本幂等，可重复执行
-- ============================================================================

BEGIN;

-- ============================================================
-- 初始库存：原材料仓 WH-RAW
-- ============================================================

-- PRD-RM-001（原材料C/钢材）: 500 KG → WH-RAW-A01
-- 先插入 stock_ledger
INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no,
                          quantity, reserved_qty, available_qty, unit_cost, received_date)
SELECT
    p.product_id,
    w.id,
    z.id,
    b.id,
    'BATCH-RAW-001',
    500.000000, 0, 500.000000, 50.00, CURRENT_DATE
FROM products p
CROSS JOIN warehouses w
CROSS JOIN zones z
CROSS JOIN bins b
WHERE p.product_code = 'PRD-RM-001'
  AND w.code = 'WH-RAW'
  AND z.warehouse_id = w.id AND z.code = 'WH-RAW-Z01'
  AND b.zone_id = z.id AND b.code = 'WH-RAW-A01'
ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, '')) DO NOTHING;

-- 库存事务记录
INSERT INTO inventory_transactions (transaction_type, product_id, warehouse_id, zone_id, bin_id,
                                    batch_no, quantity, unit_cost, source_type, source_id,
                                    remark, operator_id)
SELECT 9, -- Adjustment
    p.product_id, w.id, z.id, b.id,
    'BATCH-RAW-001', 500.000000, 50.00, 'InitialSetup', 0,
    'Q2C测试-初始库存', (SELECT user_id FROM users WHERE username = 'q2c_warehouse')
FROM products p
CROSS JOIN warehouses w
CROSS JOIN zones z
CROSS JOIN bins b
WHERE p.product_code = 'PRD-RM-001'
  AND w.code = 'WH-RAW'
  AND z.warehouse_id = w.id AND z.code = 'WH-RAW-Z01'
  AND b.zone_id = z.id AND b.code = 'WH-RAW-A01';

-- PRD-RM-002（原材料D/塑料）: 200 KG → WH-RAW-A01
INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no,
                          quantity, reserved_qty, available_qty, unit_cost, received_date)
SELECT
    p.product_id, w.id, z.id, b.id,
    'BATCH-RAW-002',
    200.000000, 0, 200.000000, 30.00, CURRENT_DATE
FROM products p
CROSS JOIN warehouses w
CROSS JOIN zones z
CROSS JOIN bins b
WHERE p.product_code = 'PRD-RM-002'
  AND w.code = 'WH-RAW'
  AND z.warehouse_id = w.id AND z.code = 'WH-RAW-Z01'
  AND b.zone_id = z.id AND b.code = 'WH-RAW-A01'
ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, '')) DO NOTHING;

INSERT INTO inventory_transactions (transaction_type, product_id, warehouse_id, zone_id, bin_id,
                                    batch_no, quantity, unit_cost, source_type, source_id,
                                    remark, operator_id)
SELECT 9,
    p.product_id, w.id, z.id, b.id,
    'BATCH-RAW-002', 200.000000, 30.00, 'InitialSetup', 0,
    'Q2C测试-初始库存', (SELECT user_id FROM users WHERE username = 'q2c_warehouse')
FROM products p
CROSS JOIN warehouses w
CROSS JOIN zones z
CROSS JOIN bins b
WHERE p.product_code = 'PRD-RM-002'
  AND w.code = 'WH-RAW'
  AND z.warehouse_id = w.id AND z.code = 'WH-RAW-Z01'
  AND b.zone_id = z.id AND b.code = 'WH-RAW-A01';

-- PRD-RM-003（辅料E/包装）: 1000 个 → WH-RAW-A02
INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, batch_no,
                          quantity, reserved_qty, available_qty, unit_cost, received_date)
SELECT
    p.product_id, w.id, z.id, b.id,
    'BATCH-RAW-003',
    1000.000000, 0, 1000.000000, 5.00, CURRENT_DATE
FROM products p
CROSS JOIN warehouses w
CROSS JOIN zones z
CROSS JOIN bins b
WHERE p.product_code = 'PRD-RM-003'
  AND w.code = 'WH-RAW'
  AND z.warehouse_id = w.id AND z.code = 'WH-RAW-Z01'
  AND b.zone_id = z.id AND b.code = 'WH-RAW-A02'
ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, '')) DO NOTHING;

INSERT INTO inventory_transactions (transaction_type, product_id, warehouse_id, zone_id, bin_id,
                                    batch_no, quantity, unit_cost, source_type, source_id,
                                    remark, operator_id)
SELECT 9,
    p.product_id, w.id, z.id, b.id,
    'BATCH-RAW-003', 1000.000000, 5.00, 'InitialSetup', 0,
    'Q2C测试-初始库存', (SELECT user_id FROM users WHERE username = 'q2c_warehouse')
FROM products p
CROSS JOIN warehouses w
CROSS JOIN zones z
CROSS JOIN bins b
WHERE p.product_code = 'PRD-RM-003'
  AND w.code = 'WH-RAW'
  AND z.warehouse_id = w.id AND z.code = 'WH-RAW-Z01'
  AND b.zone_id = z.id AND b.code = 'WH-RAW-A02';

-- ============================================================
-- 验证：成品仓 WH-FG 无初始库存（测试必须走生产流程）
-- 不插入任何 stock_ledger 记录
-- ============================================================

COMMIT;

-- ============================================================
-- 验证查询（不执行，仅供人工检查）
-- ============================================================

-- SELECT p.product_code, w.code AS warehouse, sl.quantity, sl.available_qty, sl.batch_no
-- FROM stock_ledger sl
-- JOIN products p ON sl.product_id = p.product_id
-- JOIN warehouses w ON sl.warehouse_id = w.id
-- WHERE p.product_code LIKE 'PRD-%' AND w.code LIKE 'WH-%'
-- ORDER BY w.code, p.product_code;
