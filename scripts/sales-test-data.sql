-- ============================================================
-- 销售模块完整测试数据
-- 覆盖：报价单、销售订单、发货申请、销售退货、对账单
-- 依赖基础数据：products, customers, customer_contacts, users
-- 执行方式：psql "$DATABASE_URL" -f scripts/sales-test-data.sql
-- ============================================================

BEGIN;

-- 先清理已有测试数据（按外键依赖逆序删除）
DELETE FROM reconciliation_items WHERE reconciliation_id IN (SELECT id FROM reconciliations WHERE doc_number LIKE 'SALES-TEST-%');
DELETE FROM reconciliations WHERE doc_number LIKE 'SALES-TEST-%';
DELETE FROM sales_return_items WHERE return_id IN (SELECT id FROM sales_returns WHERE doc_number LIKE 'SALES-TEST-%');
DELETE FROM sales_returns WHERE doc_number LIKE 'SALES-TEST-%';
DELETE FROM shipping_request_items WHERE shipping_request_id IN (SELECT id FROM shipping_requests WHERE doc_number LIKE 'SALES-TEST-%');
DELETE FROM shipping_requests WHERE doc_number LIKE 'SALES-TEST-%';
DELETE FROM sales_order_items WHERE order_id IN (SELECT id FROM sales_orders WHERE doc_number LIKE 'SALES-TEST-%');
DELETE FROM sales_orders WHERE doc_number LIKE 'SALES-TEST-%';
DELETE FROM quotation_items WHERE quotation_id IN (SELECT id FROM quotations WHERE doc_number LIKE 'SALES-TEST-%');
DELETE FROM quotations WHERE doc_number LIKE 'SALES-TEST-%';

-- ============================================================
-- 1. 报价单（quotations）— 覆盖全部 5 种状态
-- ============================================================

-- 1.1 报价单-草稿 (status=1 Draft)
INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, quotation_date, valid_until, status, total_amount, total_cost, estimated_margin, payment_terms, delivery_terms, remark, operator_id)
VALUES ('SALES-TEST-QUO-001', 1, 1, 6, '2026-06-01', '2026-07-01', 1,
        550.00, 300.00, 45.45, '30天净额', 'FOB 深圳', '测试报价单-草稿', 6);

-- 1.2 报价单-已发送 (status=2 Sent)
INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, quotation_date, valid_until, status, total_amount, total_cost, estimated_margin, payment_terms, delivery_terms, remark, operator_id)
VALUES ('SALES-TEST-QUO-002', 3, 6, 6, '2026-06-02', '2026-07-02', 2,
        1200.00, 700.00, 41.67, '月结30天', 'FOB 广州', '测试报价单-已发送', 6);

-- 1.3 报价单-已接受 (status=3 Accepted)
INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, quotation_date, valid_until, status, total_amount, total_cost, estimated_margin, payment_terms, delivery_terms, remark, operator_id)
VALUES ('SALES-TEST-QUO-003', 7, 10, 6, '2026-05-20', '2026-06-20', 3,
        3300.00, 1800.00, 45.45, '月结60天', 'EXW 深圳', '测试报价单-已接受', 6);

-- 1.4 报价单-已拒绝 (status=4 Rejected)
INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, quotation_date, valid_until, status, total_amount, total_cost, estimated_margin, payment_terms, delivery_terms, remark, operator_id)
VALUES ('SALES-TEST-QUO-004', 1, 1, 6, '2026-05-15', '2026-06-15', 4,
        800.00, 500.00, 37.50, '30天净额', 'FOB 深圳', '测试报价单-已拒绝', 6);

-- 1.5 报价单-已过期 (status=5 Expired)
INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, quotation_date, valid_until, status, total_amount, total_cost, estimated_margin, payment_terms, delivery_terms, remark, operator_id)
VALUES ('SALES-TEST-QUO-005', 3, 6, 6, '2026-04-01', '2026-05-01', 5,
        960.00, 600.00, 37.50, '月结30天', 'FOB 广州', '测试报价单-已过期', 6);

-- 报价单明细
-- QUO-001 明细 (草稿)
INSERT INTO quotation_items (quotation_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date)
VALUES ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-001'), 1, 13196, '防水电源/12V-600W/POWER LED', 10, 'pcs', 25.00, 15.00, 0.00, 250.00, '2026-06-15'),
       ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-001'), 2, 13197, 'B25防雨电源400-12V(MG)', 10, 'pcs', 30.00, 15.00, 0.00, 300.00, '2026-06-15');

-- QUO-002 明细 (已发送)
INSERT INTO quotation_items (quotation_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date)
VALUES ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-002'), 1, 13196, '防水电源/12V-600W/POWER LED', 20, 'pcs', 25.00, 15.00, 0.00, 500.00, '2026-07-01'),
       ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-002'), 2, 13198, 'C25防雨电源400-12V(DINAMO)', 20, 'pcs', 35.00, 20.00, 0.00, 700.00, '2026-07-01');

-- QUO-003 明细 (已接受)
INSERT INTO quotation_items (quotation_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date)
VALUES ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-003'), 1, 13196, '防水电源/12V-600W/POWER LED', 50, 'pcs', 25.00, 15.00, 0.00, 1250.00, '2026-06-01'),
       ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-003'), 2, 13197, 'B25防雨电源400-12V(MG)', 50, 'pcs', 30.00, 15.00, 0.00, 1500.00, '2026-06-01'),
       ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-003'), 3, 13198, 'C25防雨电源400-12V(DINAMO)', 20, 'pcs', 27.50, 15.00, 0.00, 550.00, '2026-06-01');

-- QUO-004 明细 (已拒绝)
INSERT INTO quotation_items (quotation_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date)
VALUES ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-004'), 1, 13196, '防水电源/12V-600W/POWER LED', 20, 'pcs', 40.00, 25.00, 0.00, 800.00, NULL);

-- QUO-005 明细 (已过期)
INSERT INTO quotation_items (quotation_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date)
VALUES ((SELECT id FROM quotations WHERE doc_number='SALES-TEST-QUO-005'), 1, 13197, 'B25防雨电源400-12V(MG)', 30, 'pcs', 32.00, 20.00, 0.00, 960.00, NULL);


-- ============================================================
-- 2. 销售订单（sales_orders）— 覆盖全部 7 种状态
-- ============================================================

-- 2.1 订单-草稿 (status=1)
INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
VALUES ('SALES-TEST-SO-001', 1, 1, 6, '2026-06-05', 1,
        550.00, 300.00, '30天净额', 'FOB 深圳', '深圳市南山区科技园', '测试订单-草稿', 6);

-- 2.2 订单-已确认 (status=2)
INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
VALUES ('SALES-TEST-SO-002', 3, 6, 6, '2026-06-03', 2,
        1200.00, 700.00, '月结30天', 'FOB 广州', '广州市天河区天河路', '测试订单-已确认', 6);

-- 2.3 订单-生产中 (status=3)
INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
VALUES ('SALES-TEST-SO-003', 7, 10, 6, '2026-05-28', 3,
        3300.00, 1800.00, '月结60天', 'EXW 深圳', '深圳市南山区科技园', '测试订单-生产中', 6);

-- 2.4 订单-部分发货 (status=4)
INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
VALUES ('SALES-TEST-SO-004', 3, 6, 6, '2026-05-25', 4,
        800.00, 450.00, '月结30天', 'FOB 广州', '广州市天河区天河路', '测试订单-部分发货', 6);

-- 2.5 订单-已发货 (status=5)
INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
VALUES ('SALES-TEST-SO-005', 7, 10, 6, '2026-05-20', 5,
        960.00, 550.00, '月结30天', 'EXW 深圳', '深圳市南山区科技园', '测试订单-已发货', 6);

-- 2.6 订单-已完成 (status=6)
INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
VALUES ('SALES-TEST-SO-006', 1, 1, 6, '2026-05-10', 6,
        440.00, 250.00, '30天净额', 'FOB 深圳', '深圳市南山区科技园', '测试订单-已完成', 6);

-- 2.7 订单-已取消 (status=7)
INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
VALUES ('SALES-TEST-SO-007', 3, 6, 6, '2026-05-15', 7,
        1500.00, 800.00, '月结30天', 'FOB 广州', '广州市天河区天河路', '测试订单-已取消', 6);

-- 订单明细
-- SO-001 (草稿) - 2行
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty)
VALUES ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-001'), 1, 13196, '防水电源/12V-600W/POWER LED', 10, 'pcs', 25.00, 15.00, 0.00, 250.00, 0, 0),
       ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-001'), 2, 13197, 'B25防雨电源400-12V(MG)', 10, 'pcs', 30.00, 15.00, 0.00, 300.00, 0, 0);

-- SO-002 (已确认) - 2行
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty)
VALUES ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-002'), 1, 13196, '防水电源/12V-600W/POWER LED', 20, 'pcs', 25.00, 15.00, 0.00, 500.00, 0, 0),
       ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-002'), 2, 13198, 'C25防雨电源400-12V(DINAMO)', 20, 'pcs', 35.00, 20.00, 0.00, 700.00, 0, 0);

-- SO-003 (生产中) - 3行
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty)
VALUES ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003'), 1, 13196, '防水电源/12V-600W/POWER LED', 50, 'pcs', 25.00, 15.00, 0.00, 1250.00, 0, 0),
       ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003'), 2, 13197, 'B25防雨电源400-12V(MG)', 50, 'pcs', 30.00, 15.00, 0.00, 1500.00, 0, 0),
       ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003'), 3, 13198, 'C25防雨电源400-12V(DINAMO)', 20, 'pcs', 27.50, 15.00, 0.00, 550.00, 0, 0);

-- SO-004 (部分发货) - 2行, 部分已发货
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty)
VALUES ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-004'), 1, 13196, '防水电源/12V-600W/POWER LED', 20, 'pcs', 25.00, 15.00, 0.00, 500.00, 10, 0),
       ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-004'), 2, 13197, 'B25防雨电源400-12V(MG)', 10, 'pcs', 30.00, 15.00, 0.00, 300.00, 0, 0);

-- SO-005 (已发货) - 1行, 全部已发货
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty)
VALUES ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), 1, 13196, '防水电源/12V-600W/POWER LED', 30, 'pcs', 32.00, 18.33, 0.00, 960.00, 30, 0);

-- SO-006 (已完成) - 1行, 全部发货+退货0
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty)
VALUES ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-006'), 1, 13197, 'B25防雨电源400-12V(MG)', 10, 'pcs', 44.00, 25.00, 0.00, 440.00, 10, 0);

-- SO-007 (已取消) - 2行
INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty)
VALUES ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-007'), 1, 13196, '防水电源/12V-600W/POWER LED', 30, 'pcs', 25.00, 15.00, 0.00, 750.00, 0, 0),
       ((SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-007'), 2, 13198, 'C25防雨电源400-12V(DINAMO)', 20, 'pcs', 37.50, 20.00, 0.00, 750.00, 0, 0);


-- ============================================================
-- 3. 发货申请（shipping_requests）— 覆盖全部 5 种状态
-- ============================================================

-- 3.1 发货-草稿 (status=1)
INSERT INTO shipping_requests (doc_number, order_id, customer_id, request_date, expected_ship_date, status, shipping_address, carrier, tracking_number, remark, operator_id)
VALUES ('SALES-TEST-SR-001', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-002'), 3, '2026-06-05', '2026-06-08', 1,
        '广州市天河区天河路', '', '', '测试发货-草稿', 6);

-- 3.2 发货-已确认 (status=2)
INSERT INTO shipping_requests (doc_number, order_id, customer_id, request_date, expected_ship_date, status, shipping_address, carrier, tracking_number, remark, operator_id)
VALUES ('SALES-TEST-SR-002', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003'), 7, '2026-06-01', '2026-06-05', 2,
        '深圳市南山区科技园', '顺丰速运', '', '测试发货-已确认', 6);

-- 3.3 发货-拣货中 (status=3)
INSERT INTO shipping_requests (doc_number, order_id, customer_id, request_date, expected_ship_date, status, shipping_address, carrier, tracking_number, remark, operator_id)
VALUES ('SALES-TEST-SR-003', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-004'), 3, '2026-05-28', '2026-06-01', 3,
        '广州市天河区天河路', '中通快递', '', '测试发货-拣货中', 6);

-- 3.4 发货-已发出 (status=4)
INSERT INTO shipping_requests (doc_number, order_id, customer_id, request_date, expected_ship_date, status, shipping_address, carrier, tracking_number, remark, operator_id)
VALUES ('SALES-TEST-SR-004', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), 7, '2026-05-22', '2026-05-25', 4,
        '深圳市南山区科技园', '顺丰速运', 'SF20260522001', '测试发货-已发出', 6);

-- 3.5 发货-已取消 (status=5)
INSERT INTO shipping_requests (doc_number, order_id, customer_id, request_date, expected_ship_date, status, shipping_address, carrier, tracking_number, remark, operator_id)
VALUES ('SALES-TEST-SR-005', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-007'), 3, '2026-05-18', '2026-05-20', 5,
        '广州市天河区天河路', '', '', '测试发货-已取消', 6);

-- 发货明细
-- SR-001 (草稿) - 对应 SO-002 的明细
INSERT INTO shipping_request_items (shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description)
SELECT (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-001'), 1, soi.id, soi.product_id, 23320, soi.quantity, 0, soi.description
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-002') AND soi.line_no = 1;

INSERT INTO shipping_request_items (shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description)
SELECT (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-001'), 2, soi.id, soi.product_id, 23320, soi.quantity, 0, soi.description
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-002') AND soi.line_no = 2;

-- SR-002 (已确认) - 对应 SO-003 的前2行
INSERT INTO shipping_request_items (shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description)
SELECT (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-002'), 1, soi.id, soi.product_id, 23320, 50, 0, soi.description
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003') AND soi.line_no = 1;

INSERT INTO shipping_request_items (shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description)
SELECT (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-002'), 2, soi.id, soi.product_id, 23320, 50, 0, soi.description
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003') AND soi.line_no = 2;

-- SR-003 (拣货中) - 对应 SO-004 的第1行(部分发货)
INSERT INTO shipping_request_items (shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description)
SELECT (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-003'), 1, soi.id, soi.product_id, 23320, 10, 0, soi.description
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-004') AND soi.line_no = 1;

-- SR-004 (已发出) - 对应 SO-005, 全部发货
INSERT INTO shipping_request_items (shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description)
SELECT (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), 1, soi.id, soi.product_id, 23320, 30, 30, soi.description
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005') AND soi.line_no = 1;


-- ============================================================
-- 4. 销售退货（sales_returns）— 覆盖全部 7 种状态
-- ============================================================

-- 4.1 退货-草稿 (status=1)
INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id)
VALUES ('SALES-TEST-RMA-001', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-004'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-003'), 3,
        '2026-06-05', 1, '尺寸不符', 250.00, '测试退货-草稿', 6);

-- 4.2 退货-已确认 (status=2)
INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id)
VALUES ('SALES-TEST-RMA-002', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), 7,
        '2026-06-03', 2, '质量问题', 320.00, '测试退货-已确认', 6);

-- 4.3 退货-已收到 (status=3)
INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id)
VALUES ('SALES-TEST-RMA-003', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), 7,
        '2026-06-01', 3, '外观损坏', 640.00, '测试退货-已收到', 6);

-- 4.4 退货-检验中 (status=4)
INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id)
VALUES ('SALES-TEST-RMA-004', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-006'), 0, 1,
        '2026-05-28', 4, '功能异常', 176.00, '测试退货-检验中', 6);

-- 4.5 退货-已完成 (status=5)
INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id)
VALUES ('SALES-TEST-RMA-005', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-006'), 0, 1,
        '2026-05-20', 5, '客户取消订单', 88.00, '测试退货-已完成', 6);

-- 4.6 退货-已取消 (status=6)
INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id)
VALUES ('SALES-TEST-RMA-006', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003'), 0, 7,
        '2026-06-02', 6, '误操作', 500.00, '测试退货-已取消', 6);

-- 4.7 退货-已拒绝 (status=7)
INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id)
VALUES ('SALES-TEST-RMA-007', (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003'), 0, 7,
        '2026-06-01', 7, '超期退货', 275.00, '测试退货-已拒绝', 6);

-- 退货明细
-- RMA-001 (草稿) - 退货处置=退货入库(1)
INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-001'), soi.id, soi.product_id, 5, 25.00, 125.00, 1
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-004') AND soi.line_no = 1;

INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-001'), soi.id, soi.product_id, 5, 25.00, 125.00, 2
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-004') AND soi.line_no = 2;

-- RMA-002 (已确认)
INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-002'), soi.id, soi.product_id, 10, 32.00, 320.00, 1
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005') AND soi.line_no = 1;

-- RMA-003 (已收到)
INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-003'), soi.id, soi.product_id, 20, 32.00, 640.00, 3
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005') AND soi.line_no = 1;

-- RMA-004 (检验中)
INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-004'), soi.id, soi.product_id, 4, 44.00, 176.00, 1
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-006') AND soi.line_no = 1;

-- RMA-005 (已完成)
INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-005'), soi.id, soi.product_id, 2, 44.00, 88.00, 2
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-006') AND soi.line_no = 1;

-- RMA-006 (已取消)
INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-006'), soi.id, soi.product_id, 10, 25.00, 250.00, 1
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003') AND soi.line_no = 1;

INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-006'), soi.id, soi.product_id, 10, 25.00, 250.00, 1
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003') AND soi.line_no = 2;

-- RMA-007 (已拒绝)
INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
SELECT (SELECT id FROM sales_returns WHERE doc_number='SALES-TEST-RMA-007'), soi.id, soi.product_id, 10, 27.50, 275.00, 1
FROM sales_order_items soi WHERE soi.order_id = (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-003') AND soi.line_no = 3;


-- ============================================================
-- 5. 对账单（reconciliations）— 覆盖全部 5 种状态
-- ============================================================

-- 5.1 对账-草稿 (status=1)
INSERT INTO reconciliations (doc_number, customer_id, period, status, total_amount, confirmed_amount, difference, remark, operator_id)
VALUES ('SALES-TEST-REC-001', 3, '2026-06', 1, 960.00, 0, 0, '测试对账-草稿', 6);

-- 5.2 对账-已发送 (status=2)
INSERT INTO reconciliations (doc_number, customer_id, period, status, total_amount, confirmed_amount, difference, remark, operator_id)
VALUES ('SALES-TEST-REC-002', 7, '2026-06', 2, 960.00, 0, 0, '测试对账-已发送', 6);

-- 5.3 对账-已确认 (status=3)
INSERT INTO reconciliations (doc_number, customer_id, period, status, total_amount, confirmed_amount, difference, remark, operator_id)
VALUES ('SALES-TEST-REC-003', 3, '2026-05', 3, 960.00, 960.00, 0, '测试对账-已确认', 6);

-- 5.4 对账-有异议 (status=4)
INSERT INTO reconciliations (doc_number, customer_id, period, status, total_amount, confirmed_amount, difference, remark, operator_id)
VALUES ('SALES-TEST-REC-004', 1, '2026-05', 4, 440.00, 220.00, 220.00, '测试对账-有异议', 6);

-- 5.5 对账-已结算 (status=5)
INSERT INTO reconciliations (doc_number, customer_id, period, status, total_amount, confirmed_amount, difference, remark, operator_id)
VALUES ('SALES-TEST-REC-005', 1, '2026-04', 5, 3300.00, 3300.00, 0, '测试对账-已结算', 6);

-- 对账明细
-- REC-001 (草稿) - 基于 SO-002 + SR-001
INSERT INTO reconciliation_items (reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed, remark)
VALUES
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-001'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-001'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-002'), 13196, 20, 25.00, 500.00, false, NULL),
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-001'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-001'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-002'), 13198, 20, 23.00, 460.00, false, NULL);

-- REC-002 (已发送) - 基于 SO-005 + SR-004
INSERT INTO reconciliation_items (reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed, remark)
VALUES
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-002'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), 13196, 30, 32.00, 960.00, false, NULL);

-- REC-003 (已确认)
INSERT INTO reconciliation_items (reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed, remark)
VALUES
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-003'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), 13196, 30, 32.00, 960.00, true, '客户已确认');

-- REC-004 (有异议)
INSERT INTO reconciliation_items (reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed, remark)
VALUES
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-004'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-006'), 13197, 10, 44.00, 440.00, false, '价格有差异');

-- REC-005 (已结算)
INSERT INTO reconciliation_items (reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed, remark)
VALUES
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-005'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), 13196, 30, 32.00, 960.00, true, NULL),
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-005'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), 13197, 50, 30.00, 1500.00, true, NULL),
((SELECT id FROM reconciliations WHERE doc_number='SALES-TEST-REC-005'), (SELECT id FROM shipping_requests WHERE doc_number='SALES-TEST-SR-004'), (SELECT id FROM sales_orders WHERE doc_number='SALES-TEST-SO-005'), 13198, 30, 28.00, 840.00, true, NULL);


COMMIT;

-- 验证插入结果
SELECT '=== 报价单 ===' AS section;
SELECT doc_number, customer_id, status, total_amount FROM quotations WHERE doc_number LIKE 'SALES-TEST-QUO-%' ORDER BY doc_number;

SELECT '=== 销售订单 ===' AS section;
SELECT doc_number, customer_id, status, total_amount FROM sales_orders WHERE doc_number LIKE 'SALES-TEST-SO-%' ORDER BY doc_number;

SELECT '=== 发货申请 ===' AS section;
SELECT doc_number, customer_id, status FROM shipping_requests WHERE doc_number LIKE 'SALES-TEST-SR-%' ORDER BY doc_number;

SELECT '=== 销售退货 ===' AS section;
SELECT doc_number, customer_id, status, total_amount FROM sales_returns WHERE doc_number LIKE 'SALES-TEST-RMA-%' ORDER BY doc_number;

SELECT '=== 对账单 ===' AS section;
SELECT doc_number, customer_id, period, status, total_amount FROM reconciliations WHERE doc_number LIKE 'SALES-TEST-REC-%' ORDER BY doc_number;
