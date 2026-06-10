-- ============================================================================
-- Q2C E2E 测试 — 主数据
-- 物料、BOM、工艺路线、客户、供应商、仓库、仓位、价格
-- 脚本幂等，可重复执行
-- ============================================================================

BEGIN;

-- ============================================================
-- 1. 物料主数据
-- ============================================================

-- 成品 A
INSERT INTO products (product_code, pdt_name, unit, status, meta)
VALUES ('PRD-FG-001', '成品A（产成品）', '个', 1,
        '{"specification":"标准成品","acquire_channel":"self-made","product_type":"finished_goods"}')
ON CONFLICT (product_code) WHERE deleted_at IS NULL DO NOTHING;

-- 半成品 B
INSERT INTO products (product_code, pdt_name, unit, status, meta)
VALUES ('PRD-SFG-001', '半成品B', '个', 1,
        '{"specification":"标准半成品","acquire_channel":"self-made","product_type":"semi_finished"}')
ON CONFLICT (product_code) WHERE deleted_at IS NULL DO NOTHING;

-- 原材料 C（钢材）
INSERT INTO products (product_code, pdt_name, unit, status, meta)
VALUES ('PRD-RM-001', '原材料C（钢材）', 'KG', 1,
        '{"specification":"Q235B钢材","acquire_channel":"purchase","product_type":"raw_material"}')
ON CONFLICT (product_code) WHERE deleted_at IS NULL DO NOTHING;

-- 原材料 D（塑料）
INSERT INTO products (product_code, pdt_name, unit, status, meta)
VALUES ('PRD-RM-002', '原材料D（塑料）', 'KG', 1,
        '{"specification":"PP塑料粒子","acquire_channel":"purchase","product_type":"raw_material"}')
ON CONFLICT (product_code) WHERE deleted_at IS NULL DO NOTHING;

-- 辅料 E（包装）
INSERT INTO products (product_code, pdt_name, unit, status, meta)
VALUES ('PRD-RM-003', '辅料E（包装）', '个', 1,
        '{"specification":"标准包装箱","acquire_channel":"purchase","product_type":"consumable"}')
ON CONFLICT (product_code) WHERE deleted_at IS NULL DO NOTHING;

-- ============================================================
-- 2. 客户
-- ============================================================

-- CUS-001: 正常客户
INSERT INTO customers (customer_code, customer_name, short_name, category, status,
                       tax_number, invoice_title, credit_limit, payment_terms,
                       remark, operator_id)
VALUES ('CUS-001', '测试客户A', '客户A', 2, 2,
        '91110000MA01ABCD01', '测试客户A有限公司', 500000.00, 'NET 30',
        'Q2C测试客户-正常', (SELECT user_id FROM users WHERE username = 'q2c_sales' LIMIT 1))
ON CONFLICT (customer_code) WHERE deleted_at IS NULL DO NOTHING;

-- CUS-001 联系人
INSERT INTO customer_contacts (customer_id, contact_name, position, phone, email, is_primary)
SELECT c.customer_id, '张三', '采购经理', '13800000001', 'zhangsan@test-a.com', true
FROM customers c WHERE c.customer_code = 'CUS-001'
ON CONFLICT DO NOTHING;

-- CUS-001 地址
INSERT INTO customer_addresses (customer_id, address_type, province, city, district, detail, contact_name, contact_phone, is_default)
SELECT c.customer_id, 'shipping', '上海市', '上海市', '浦东新区', '张江高科技园区xxx号',
       '张三', '13800000001', true
FROM customers c WHERE c.customer_code = 'CUS-001'
ON CONFLICT DO NOTHING;

-- CUS-002: 信用冻结客户
INSERT INTO customers (customer_code, customer_name, short_name, category, status,
                       tax_number, invoice_title, credit_limit, payment_terms,
                       remark, operator_id)
VALUES ('CUS-002', '测试客户B（信用冻结）', '客户B', 2, 4,
        '91110000MA01ABCD02', '测试客户B有限公司', 0.00, 'NET 30',
        'Q2C测试客户-信用冻结', (SELECT user_id FROM users WHERE username = 'q2c_sales' LIMIT 1))
ON CONFLICT (customer_code) WHERE deleted_at IS NULL DO NOTHING;

-- CUS-002 联系人
INSERT INTO customer_contacts (customer_id, contact_name, position, phone, email, is_primary)
SELECT c.customer_id, '李四', '采购员', '13800000002', 'lisi@test-b.com', true
FROM customers c WHERE c.customer_code = 'CUS-002'
ON CONFLICT DO NOTHING;

-- ============================================================
-- 3. 供应商
-- ============================================================

-- SUP-001: 主力供应商
INSERT INTO suppliers (supplier_code, supplier_name, short_name, category, status,
                       tax_number, lead_time_days, payment_terms,
                       remark, operator_id)
VALUES ('SUP-001', '测试供应商A', '供应商A', 1, 2,
        '91110000MA01EFGH01', 7, 'NET 45',
        'Q2C测试供应商-主力', (SELECT user_id FROM users WHERE username = 'q2c_buyer' LIMIT 1))
ON CONFLICT (supplier_code) WHERE deleted_at IS NULL DO NOTHING;

-- SUP-001 联系人
INSERT INTO supplier_contacts (supplier_id, contact_name, position, phone, email, is_primary)
SELECT s.supplier_id, '王五', '销售经理', '13900000001', 'wangwu@sup-a.com', true
FROM suppliers s WHERE s.supplier_code = 'SUP-001'
ON CONFLICT DO NOTHING;

-- SUP-001 银行账户
INSERT INTO supplier_bank_accounts (supplier_id, bank_name, account_name, account_number, is_default)
SELECT s.supplier_id, '工商银行', '测试供应商A有限公司', '6222000000000001234', true
FROM suppliers s WHERE s.supplier_code = 'SUP-001'
ON CONFLICT DO NOTHING;

-- SUP-002: 备选供应商
INSERT INTO suppliers (supplier_code, supplier_name, short_name, category, status,
                       tax_number, lead_time_days, payment_terms,
                       remark, operator_id)
VALUES ('SUP-002', '测试供应商B', '供应商B', 1, 3,
        '91110000MA01EFGH02', 14, 'NET 30',
        'Q2C测试供应商-备选', (SELECT user_id FROM users WHERE username = 'q2c_buyer' LIMIT 1))
ON CONFLICT (supplier_code) WHERE deleted_at IS NULL DO NOTHING;

-- ============================================================
-- 4. 仓库
-- ============================================================

INSERT INTO warehouses (code, name, warehouse_type, status, address, operator_id)
VALUES
    ('WH-RAW',   'Q2C原材料仓',   1, 1, 'A区-原材料库',  (SELECT user_id FROM users WHERE username = 'q2c_warehouse')),
    ('WH-WIP',   'Q2C在制品仓',   3, 1, 'B区-在制品库',  (SELECT user_id FROM users WHERE username = 'q2c_warehouse')),
    ('WH-FG',    'Q2C成品仓',     2, 1, 'C区-成品库',    (SELECT user_id FROM users WHERE username = 'q2c_warehouse')),
    ('WH-QC',    'Q2C待检仓',     4, 1, 'D区-待检区',    (SELECT user_id FROM users WHERE username = 'q2c_warehouse')),
    ('WH-REJ',   'Q2C隔离仓',     4, 1, 'E区-隔离区',    (SELECT user_id FROM users WHERE username = 'q2c_warehouse')),
    ('WH-SCRAP', 'Q2C废品仓',     4, 1, 'F区-废品区',    (SELECT user_id FROM users WHERE username = 'q2c_warehouse'))
ON CONFLICT (code) DO NOTHING;

-- ============================================================
-- 5. 库区 + 库位
-- ============================================================

-- WH-RAW 库区 + 库位
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order)
SELECT w.id, 'WH-RAW-Z01', '原材料存储区', 2, 1
FROM warehouses w WHERE w.code = 'WH-RAW'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-RAW-A01', '原材料A01', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-RAW' AND z.code = 'WH-RAW-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-RAW-A02', '原材料A02', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-RAW' AND z.code = 'WH-RAW-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

-- WH-WIP 库区 + 库位
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order)
SELECT w.id, 'WH-WIP-Z01', '在制品存储区', 2, 1
FROM warehouses w WHERE w.code = 'WH-WIP'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-WIP-B01', '在制品B01', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-WIP' AND z.code = 'WH-WIP-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

-- WH-FG 库区 + 库位
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order)
SELECT w.id, 'WH-FG-Z01', '成品存储区', 2, 1
FROM warehouses w WHERE w.code = 'WH-FG'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-FG-C01', '成品C01', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-FG' AND z.code = 'WH-FG-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-FG-C02', '成品C02', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-FG' AND z.code = 'WH-FG-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

-- WH-QC 库区 + 库位
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order)
SELECT w.id, 'WH-QC-Z01', '待检区', 5, 1
FROM warehouses w WHERE w.code = 'WH-QC'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-QC-D01', '待检D01', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-QC' AND z.code = 'WH-QC-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

-- WH-REJ 库区 + 库位
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order)
SELECT w.id, 'WH-REJ-Z01', '隔离区', 5, 1
FROM warehouses w WHERE w.code = 'WH-REJ'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-REJ-E01', '隔离E01', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-REJ' AND z.code = 'WH-REJ-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

-- WH-SCRAP 库区 + 库位
INSERT INTO zones (warehouse_id, code, name, zone_type, sort_order)
SELECT w.id, 'WH-SCRAP-Z01', '废品区', 5, 1
FROM warehouses w WHERE w.code = 'WH-SCRAP'
ON CONFLICT (warehouse_id, code) DO NOTHING;

INSERT INTO bins (zone_id, code, name, status)
SELECT z.id, 'WH-SCRAP-F01', '废品F01', 1
FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
WHERE w.code = 'WH-SCRAP' AND z.code = 'WH-SCRAP-Z01'
ON CONFLICT (zone_id, code) DO NOTHING;

-- ============================================================
-- 6. BOM
-- ============================================================

-- 成品A 的 BOM
INSERT INTO boms (bom_name, bom_detail, bom_category_id, status, version, published_at, created_by)
VALUES ('成品A-BOM',
        '{"nodes":[{"product_code":"PRD-SFG-001","quantity":1,"unit":"个"},{"product_code":"PRD-RM-002","quantity":0.5,"unit":"KG"},{"product_code":"PRD-RM-003","quantity":1,"unit":"个"}]}',
        NULL, 2, 1, NOW(),
        (SELECT user_id FROM users WHERE username = 'q2c_prod_mgr' LIMIT 1))
ON CONFLICT DO NOTHING;

-- 获取成品A BOM ID
-- 成品A BOM 子件（bom_nodes）
-- 半成品B × 1
INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, order_num, unit)
SELECT b.bom_id,
       (SELECT product_id FROM products WHERE product_code = 'PRD-SFG-001'),
       'PRD-SFG-001', 1.000000, 0, 0.0000, 1, '个'
FROM boms b WHERE b.bom_name = '成品A-BOM'
ON CONFLICT DO NOTHING;

-- 原材料D × 0.5 KG
INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, order_num, unit)
SELECT b.bom_id,
       (SELECT product_id FROM products WHERE product_code = 'PRD-RM-002'),
       'PRD-RM-002', 0.500000, 0, 0.0000, 2, 'KG'
FROM boms b WHERE b.bom_name = '成品A-BOM'
ON CONFLICT DO NOTHING;

-- 辅料E × 1
INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, order_num, unit)
SELECT b.bom_id,
       (SELECT product_id FROM products WHERE product_code = 'PRD-RM-003'),
       'PRD-RM-003', 1.000000, 0, 0.0000, 3, '个'
FROM boms b WHERE b.bom_name = '成品A-BOM'
ON CONFLICT DO NOTHING;

-- 半成品B 的 BOM
INSERT INTO boms (bom_name, bom_detail, bom_category_id, status, version, published_at, created_by)
VALUES ('半成品B-BOM',
        '{"nodes":[{"product_code":"PRD-RM-001","quantity":2,"unit":"KG"}]}',
        NULL, 2, 1, NOW(),
        (SELECT user_id FROM users WHERE username = 'q2c_prod_mgr' LIMIT 1))
ON CONFLICT DO NOTHING;

-- 半成品B BOM 子件：原材料C × 2 KG
INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, order_num, unit)
SELECT b.bom_id,
       (SELECT product_id FROM products WHERE product_code = 'PRD-RM-001'),
       'PRD-RM-001', 2.000000, 0, 0.0000, 1, 'KG'
FROM boms b WHERE b.bom_name = '半成品B-BOM'
ON CONFLICT DO NOTHING;

-- ============================================================
-- 7. 工艺路线
-- ============================================================

-- 成品A 工艺路线
INSERT INTO routings (name, description, operator_id)
VALUES ('成品A-工艺路线', '注塑→组装→检验',
        (SELECT user_id FROM users WHERE username = 'q2c_prod_mgr' LIMIT 1))
ON CONFLICT DO NOTHING;

-- 成品A 工艺路线步骤
INSERT INTO routing_steps (routing_id, process_code, step_order, is_required)
SELECT r.id, 'INJECTION_MOLDING', 10, true
FROM routings r WHERE r.name = '成品A-工艺路线'
ON CONFLICT DO NOTHING;

INSERT INTO routing_steps (routing_id, process_code, step_order, is_required)
SELECT r.id, 'ASSEMBLY', 20, true
FROM routings r WHERE r.name = '成品A-工艺路线'
ON CONFLICT DO NOTHING;

INSERT INTO routing_steps (routing_id, process_code, step_order, is_required)
SELECT r.id, 'INSPECTION', 30, true
FROM routings r WHERE r.name = '成品A-工艺路线'
ON CONFLICT DO NOTHING;

-- 半成品B 工艺路线
INSERT INTO routings (name, description, operator_id)
VALUES ('半成品B-工艺路线', '机加工',
        (SELECT user_id FROM users WHERE username = 'q2c_prod_mgr' LIMIT 1))
ON CONFLICT DO NOTHING;

INSERT INTO routing_steps (routing_id, process_code, step_order, is_required)
SELECT r.id, 'MACHINING', 10, true
FROM routings r WHERE r.name = '半成品B-工艺路线'
ON CONFLICT DO NOTHING;

-- BOM-工艺路线关联
INSERT INTO bom_routings (product_code, routing_id, operator_id)
SELECT 'PRD-FG-001', r.id,
       (SELECT user_id FROM users WHERE username = 'q2c_prod_mgr' LIMIT 1)
FROM routings r WHERE r.name = '成品A-工艺路线'
ON CONFLICT (product_code) DO NOTHING;

INSERT INTO bom_routings (product_code, routing_id, operator_id)
SELECT 'PRD-SFG-001', r.id,
       (SELECT user_id FROM users WHERE username = 'q2c_prod_mgr' LIMIT 1)
FROM routings r WHERE r.name = '半成品B-工艺路线'
ON CONFLICT (product_code) DO NOTHING;

-- ============================================================
-- 8. 价格
-- ============================================================

-- 成品A 销售价 ¥1,500
INSERT INTO price_log (product_id, price_type, new_price, operator_id, remark)
SELECT p.product_id, 2, 1500.00,
       (SELECT user_id FROM users WHERE username = 'q2c_sales_mgr' LIMIT 1),
       'Q2C测试-标准售价'
FROM products p WHERE p.product_code = 'PRD-FG-001';

-- 成品A 标准成本 ¥800
INSERT INTO price_log (product_id, price_type, new_price, operator_id, remark)
SELECT p.product_id, 3, 800.00,
       (SELECT user_id FROM users WHERE username = 'q2c_cost_acct' LIMIT 1),
       'Q2C测试-标准成本'
FROM products p WHERE p.product_code = 'PRD-FG-001';

-- 原材料C 采购价 ¥50/KG
INSERT INTO price_log (product_id, price_type, new_price, operator_id, remark)
SELECT p.product_id, 1, 50.00,
       (SELECT user_id FROM users WHERE username = 'q2c_buyer' LIMIT 1),
       'Q2C测试-采购价'
FROM products p WHERE p.product_code = 'PRD-RM-001';

-- 原材料D 采购价 ¥30/KG
INSERT INTO price_log (product_id, price_type, new_price, operator_id, remark)
SELECT p.product_id, 1, 30.00,
       (SELECT user_id FROM users WHERE username = 'q2c_buyer' LIMIT 1),
       'Q2C测试-采购价'
FROM products p WHERE p.product_code = 'PRD-RM-002';

-- 辅料E 采购价 ¥5/个
INSERT INTO price_log (product_id, price_type, new_price, operator_id, remark)
SELECT p.product_id, 1, 5.00,
       (SELECT user_id FROM users WHERE username = 'q2c_buyer' LIMIT 1),
       'Q2C测试-采购价'
FROM products p WHERE p.product_code = 'PRD-RM-003';

COMMIT;
