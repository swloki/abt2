-- ============================================================================
-- Q2C E2E 测试 — 清理脚本
-- 按依赖关系逆序删除所有 Q2C 测试数据
-- 使用事务确保原子性
-- ============================================================================

BEGIN;

-- ============================================================
-- 1. 业务数据（报价/订单/采购/工单/发货/财务/质检）
-- ============================================================

-- 财务
DELETE FROM cash_journals WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 库存事务
DELETE FROM inventory_transactions WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 库存台账
DELETE FROM stock_ledger WHERE warehouse_id IN (
    SELECT id FROM warehouses WHERE code IN ('WH-RAW','WH-WIP','WH-FG','WH-QC','WH-REJ','WH-SCRAP')
);

-- 领料单
DELETE FROM material_requisition_items WHERE requisition_id IN (
    SELECT id FROM material_requisitions WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM material_requisitions WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 倒冲
DELETE FROM backflush_items WHERE record_id IN (
    SELECT id FROM backflush_records WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM backflush_records WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 到货通知
DELETE FROM arrival_notice_items WHERE notice_id IN (
    SELECT id FROM arrival_notices WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM arrival_notices WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 工单报工
DELETE FROM work_reports WHERE work_order_id IN (
    SELECT id FROM work_orders WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);

-- 生产检验
DELETE FROM production_inspections WHERE work_order_id IN (
    SELECT id FROM work_orders WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);

-- 生产批次（必须在 work_orders 之前删除）
DELETE FROM production_batches WHERE work_order_id IN (
    SELECT id FROM work_orders WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);

-- 工单（包括 UI 创建的，可能 operator_id 不是 q2c 用户）
DELETE FROM production_batches WHERE work_order_id IN (
    SELECT id FROM work_orders WHERE doc_number LIKE 'WO-E2E%' OR operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM work_order_routings WHERE work_order_id IN (
    SELECT id FROM work_orders WHERE doc_number LIKE 'WO-E2E%' OR operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM work_orders WHERE doc_number LIKE 'WO-E2E%' OR operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 采购
DELETE FROM purchase_order_items WHERE order_id IN (
    SELECT id FROM purchase_orders WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM purchase_orders WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 销售订单
DELETE FROM sales_order_items WHERE order_id IN (
    SELECT id FROM sales_orders WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM sales_orders WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 报价
DELETE FROM quotation_items WHERE quotation_id IN (
    SELECT id FROM quotations WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM quotations WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 盘点
DELETE FROM cycle_count_items WHERE count_id IN (
    SELECT id FROM cycle_counts WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM cycle_counts WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 调拨
DELETE FROM transfer_items WHERE transfer_id IN (
    SELECT id FROM inventory_transfers WHERE operator_id IN (
        SELECT user_id FROM users WHERE username LIKE 'q2c_%'
    )
);
DELETE FROM inventory_transfers WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 锁库
DELETE FROM inventory_locks WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 价格日志
DELETE FROM price_log WHERE operator_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- ============================================================
-- 2. 主数据（BOM → 工艺路线 → 物料 → 客户 → 供应商 → 仓库）
-- ============================================================

-- BOM
DELETE FROM bom_labor_processes WHERE product_code IN ('PRD-FG-001','PRD-SFG-001','PRD-RM-001','PRD-RM-002','PRD-RM-003');
DELETE FROM bom_routings WHERE product_code IN ('PRD-FG-001','PRD-SFG-001');
DELETE FROM bom_nodes WHERE product_code IN ('PRD-SFG-001','PRD-RM-001','PRD-RM-002','PRD-RM-003');
DELETE FROM boms WHERE bom_name IN ('成品A-BOM','半成品B-BOM');

-- 工艺路线
DELETE FROM routing_steps WHERE routing_id IN (
    SELECT id FROM routings WHERE name IN ('成品A-工艺路线','半成品B-工艺路线')
);
DELETE FROM routings WHERE name IN ('成品A-工艺路线','半成品B-工艺路线');

-- 工序字典
DELETE FROM labor_process_dicts WHERE code IN ('INJECTION_MOLDING','ASSEMBLY','INSPECTION','MACHINING');

-- 客户
DELETE FROM customer_addresses WHERE customer_id IN (
    SELECT customer_id FROM customers WHERE customer_code IN ('CUS-001','CUS-002')
);
DELETE FROM customer_contacts WHERE customer_id IN (
    SELECT customer_id FROM customers WHERE customer_code IN ('CUS-001','CUS-002')
);
DELETE FROM customers WHERE customer_code IN ('CUS-001','CUS-002');

-- 供应商
DELETE FROM supplier_bank_accounts WHERE supplier_id IN (
    SELECT supplier_id FROM suppliers WHERE supplier_code IN ('SUP-001','SUP-002')
);
DELETE FROM supplier_contacts WHERE supplier_id IN (
    SELECT supplier_id FROM suppliers WHERE supplier_code IN ('SUP-001','SUP-002')
);
DELETE FROM suppliers WHERE supplier_code IN ('SUP-001','SUP-002');

-- 仓库（先删库位 → 库区 → 仓库）
DELETE FROM bins WHERE zone_id IN (
    SELECT z.id FROM zones z JOIN warehouses w ON z.warehouse_id = w.id
    WHERE w.code IN ('WH-RAW','WH-WIP','WH-FG','WH-QC','WH-REJ','WH-SCRAP')
);
DELETE FROM zones WHERE warehouse_id IN (
    SELECT id FROM warehouses WHERE code IN ('WH-RAW','WH-WIP','WH-FG','WH-QC','WH-REJ','WH-SCRAP')
);
DELETE FROM warehouses WHERE code IN ('WH-RAW','WH-WIP','WH-FG','WH-QC','WH-REJ','WH-SCRAP');

-- 物料
DELETE FROM products WHERE product_code IN ('PRD-FG-001','PRD-SFG-001','PRD-RM-001','PRD-RM-002','PRD-RM-003');

-- ============================================================
-- 3. 用户与角色
-- ============================================================

-- 用户-角色关联
DELETE FROM user_roles WHERE user_id IN (
    SELECT user_id FROM users WHERE username LIKE 'q2c_%'
);

-- 角色权限
DELETE FROM role_permissions WHERE role_id IN (
    SELECT role_id FROM roles WHERE role_code LIKE 'q2c_%'
);

-- 用户
DELETE FROM users WHERE username LIKE 'q2c_%';

-- 角色
DELETE FROM roles WHERE role_code LIKE 'q2c_%';

-- 部门
DELETE FROM departments WHERE department_code LIKE 'Q2C_%';

COMMIT;
