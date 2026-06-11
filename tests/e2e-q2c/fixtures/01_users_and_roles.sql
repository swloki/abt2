-- ============================================================================
-- Q2C E2E 测试 — 用户与角色
-- 创建 15 个测试用户 + 对应业务角色 + 权限分配
-- 密码: test1234 (Argon2 hash，复用 tests/permission/seed.sql 的 hash)
-- 脚本幂等，可重复执行（ON CONFLICT DO NOTHING）
-- ============================================================================

BEGIN;

-- ============================================================
-- 1. 部门
-- ============================================================
INSERT INTO departments (department_name, department_code, description, is_active, is_default)
VALUES
    ('Q2C-销售部',     'Q2C_SALES',     'Q2C测试-销售团队',     true, false),
    ('Q2C-计划部',     'Q2C_PLANNING',  'Q2C测试-生产计划',     true, false),
    ('Q2C-采购部',     'Q2C_PURCHASING','Q2C测试-采购管理',     true, false),
    ('Q2C-生产部',     'Q2C_PRODUCTION','Q2C测试-生产制造',     true, false),
    ('Q2C-质量部',     'Q2C_QUALITY',   'Q2C测试-质量管理',     true, false),
    ('Q2C-仓储部',     'Q2C_WAREHOUSE', 'Q2C测试-仓库管理',     true, false),
    ('Q2C-财务部',     'Q2C_FINANCE',   'Q2C测试-财务管理',     true, false),
    ('Q2C-管理层',     'Q2C_MANAGEMENT','Q2C测试-高层管理',     true, false)
ON CONFLICT (department_code) DO NOTHING;

-- ============================================================
-- 2. 业务角色
-- ============================================================

-- 销售专员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-销售专员', 'q2c_sales_role', false, 'Q2C测试-报价/订单操作')
ON CONFLICT (role_code) DO NOTHING;

-- 销售经理
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-销售经理', 'q2c_sales_mgr_role', false, 'Q2C测试-审批/管理')
ON CONFLICT (role_code) DO NOTHING;

-- 计划员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-计划员', 'q2c_planner_role', false, 'Q2C测试-MRP/需求分解')
ON CONFLICT (role_code) DO NOTHING;

-- 采购专员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-采购专员', 'q2c_buyer_role', false, 'Q2C测试-采购操作')
ON CONFLICT (role_code) DO NOTHING;

-- 采购经理
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-采购经理', 'q2c_buyer_mgr_role', false, 'Q2C测试-采购审批')
ON CONFLICT (role_code) DO NOTHING;

-- 生产主管
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-生产主管', 'q2c_prod_mgr_role', false, 'Q2C测试-工单/排产')
ON CONFLICT (role_code) DO NOTHING;

-- 车间操作员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-操作员', 'q2c_operator_role', false, 'Q2C测试-报工')
ON CONFLICT (role_code) DO NOTHING;

-- 质检员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-质检员', 'q2c_qc_role', false, 'Q2C测试-来料/成品质检')
ON CONFLICT (role_code) DO NOTHING;

-- 质量主管
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-质量主管', 'q2c_qc_mgr_role', false, 'Q2C测试-MRB/质量审批')
ON CONFLICT (role_code) DO NOTHING;

-- 仓管员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-仓管员', 'q2c_warehouse_role', false, 'Q2C测试-收发存')
ON CONFLICT (role_code) DO NOTHING;

-- 财务会计
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-财务会计', 'q2c_accountant_role', false, 'Q2C测试-AR/AP/发票/核销')
ON CONFLICT (role_code) DO NOTHING;

-- 成本会计
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-成本会计', 'q2c_cost_acct_role', false, 'Q2C测试-成本核算')
ON CONFLICT (role_code) DO NOTHING;

-- 出纳
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-出纳', 'q2c_cashier_role', false, 'Q2C测试-收付款')
ON CONFLICT (role_code) DO NOTHING;

-- 总账会计
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-总账会计', 'q2c_gl_acct_role', false, 'Q2C测试-总账/结算')
ON CONFLICT (role_code) DO NOTHING;

-- 总经理
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('Q2C-总经理', 'q2c_gm_role', false, 'Q2C测试-会签审批')
ON CONFLICT (role_code) DO NOTHING;

-- ============================================================
-- 3. 角色权限分配
-- ============================================================

-- 3.1 销售专员: QUOTATION/SALES_ORDER/CUSTOMER/SHIPPING CRUD + PRODUCT/PRICE read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'QUOTATION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'QUOTATION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'QUOTATION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'SALES_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'SALES_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'SALES_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'CUSTOMER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'CUSTOMER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'PRICE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_role'), 'SHIPPING', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.2 销售经理: 销售专员权限 + 审批 + delete
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'QUOTATION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'QUOTATION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'QUOTATION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'QUOTATION', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'SALES_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'SALES_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'SALES_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'SALES_ORDER', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'CUSTOMER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'CUSTOMER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'CUSTOMER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'PRICE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'SHIPPING', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'SHIPPING', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_sales_mgr_role'), 'SHIPPING', 'update')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.3 计划员: BOM/PRODUCT/COST read + WORK_ORDER/MES_PLAN read/update
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_planner_role'), 'BOM', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_planner_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_planner_role'), 'SALES_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_planner_role'), 'WORK_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_planner_role'), 'WORK_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_planner_role'), 'COST', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.4 采购专员: PURCHASE_ORDER/SUPPLIER/PRICE CRUD
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'PURCHASE_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'PURCHASE_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'PURCHASE_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'SUPPLIER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'PRICE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'WAREHOUSE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'INVENTORY', 'read'),
    -- 补充: 来料通知创建需要 INVENTORY:create
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_role'), 'INVENTORY', 'create')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.5 采购经理: 采购专员权限 + delete + 审批
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_mgr_role'), 'PURCHASE_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_mgr_role'), 'PURCHASE_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_mgr_role'), 'PURCHASE_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_mgr_role'), 'PURCHASE_ORDER', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_mgr_role'), 'SUPPLIER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_mgr_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_buyer_mgr_role'), 'PRICE', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.6 生产主管: WORK_ORDER/INSPECTION/LABOR_COST CRUD + BOM/COST read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'WORK_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'WORK_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'WORK_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'INSPECTION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'INSPECTION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'INSPECTION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'LABOR_COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'LABOR_COST', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'BOM', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_prod_mgr_role'), 'INVENTORY', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.7 车间操作员: WORK_ORDER/INSPECTION/LABOR_COST read + update
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'WORK_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'WORK_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'INSPECTION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'INSPECTION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'LABOR_COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'LABOR_COST', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'PRODUCT', 'read'),
    -- 补充: 报工创建需要 WORK_ORDER:create
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_operator_role'), 'WORK_ORDER', 'create')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.8 质检员: INSPECTION/QMS CRUD + PRODUCT/WAREHOUSE read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_role'), 'INSPECTION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_role'), 'INSPECTION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_role'), 'INSPECTION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_role'), 'WAREHOUSE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_role'), 'INVENTORY', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.9 质量主管: 质检员权限 + delete + MRB
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_mgr_role'), 'INSPECTION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_mgr_role'), 'INSPECTION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_mgr_role'), 'INSPECTION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_mgr_role'), 'INSPECTION', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_mgr_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_mgr_role'), 'WAREHOUSE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_qc_mgr_role'), 'INVENTORY', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.10 仓管员: WAREHOUSE/LOCATION/INVENTORY CRUD + PRODUCT read + SHIPPING/WORK_ORDER read
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'WAREHOUSE', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'WAREHOUSE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'WAREHOUSE', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'WAREHOUSE', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'LOCATION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'LOCATION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'LOCATION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'INVENTORY', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'INVENTORY', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'INVENTORY', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'INVENTORY', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'PURCHASE_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'CATEGORY', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'SHIPPING', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'SHIPPING', 'update'),
    -- 补充: 发货创建需要 SHIPPING:create, 成品入库需要 WORK_ORDER:create
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'SHIPPING', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'WORK_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_warehouse_role'), 'WORK_ORDER', 'create')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.11 财务会计: FMS/COST/SHIPPING/SALES_ORDER/PURCHASE_ORDER CRUD
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'FMS', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'FMS', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'FMS', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'FMS', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'COST', 'update'),
    -- 补充: 对账核销创建需要 SALES_ORDER:create
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'SALES_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'SALES_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'PURCHASE_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'SHIPPING', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'CUSTOMER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_accountant_role'), 'SUPPLIER', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;
-- 3.12 成本会计: COST/PRODUCT/BOM/WORK_ORDER CRUD
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cost_acct_role'), 'COST', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cost_acct_role'), 'COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cost_acct_role'), 'COST', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cost_acct_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cost_acct_role'), 'BOM', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cost_acct_role'), 'WORK_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cost_acct_role'), 'FMS', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.13 出纳: FMS read + update (收付款)
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cashier_role'), 'FMS', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cashier_role'), 'FMS', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cashier_role'), 'FMS', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cashier_role'), 'CUSTOMER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_cashier_role'), 'SUPPLIER', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.14 总账会计: FMS 全权限
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_gl_acct_role'), 'FMS', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_gl_acct_role'), 'FMS', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_gl_acct_role'), 'FMS', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_gl_acct_role'), 'FMS', 'delete'),
    ((SELECT role_id FROM roles WHERE role_code = 'q2c_gl_acct_role'), 'COST', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 3.15 总经理: 所有资源 read + 审批权限
INSERT INTO role_permissions (role_id, resource_code, action)
SELECT r.role_id, v.resource_code, v.action
FROM roles r
CROSS JOIN (VALUES
    ('QUOTATION', 'read'), ('QUOTATION', 'update'),
    ('SALES_ORDER', 'read'), ('SALES_ORDER', 'update'),
    ('PURCHASE_ORDER', 'read'), ('PURCHASE_ORDER', 'update'),
    ('WORK_ORDER', 'read'), ('WORK_ORDER', 'update'),
    ('SHIPPING', 'read'), ('SHIPPING', 'update'),
    ('FMS', 'read'), ('FMS', 'update'),
    ('CUSTOMER', 'read'), ('SUPPLIER', 'read'),
    ('PRODUCT', 'read'), ('BOM', 'read'),
    ('COST', 'read'), ('INVENTORY', 'read'),
    ('INSPECTION', 'read')
) v(resource_code, action)
WHERE r.role_code = 'q2c_gm_role'
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- ============================================================
-- 4. 测试用户（密码: test1234）
-- ============================================================

INSERT INTO users (username, password_hash, display_name, is_super_admin, is_active) VALUES
    ('q2c_sales',      '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-销售专员',     false, true),
    ('q2c_sales_mgr',  '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-销售经理',     false, true),
    ('q2c_planner',    '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-计划员',       false, true),
    ('q2c_buyer',      '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-采购专员',     false, true),
    ('q2c_buyer_mgr',  '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-采购经理',     false, true),
    ('q2c_prod_mgr',   '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-生产主管',     false, true),
    ('q2c_operator',   '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-操作员',       false, true),
    ('q2c_qc',         '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-质检员',       false, true),
    ('q2c_qc_mgr',     '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-质量主管',     false, true),
    ('q2c_warehouse',  '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-仓管员',       false, true),
    ('q2c_accountant', '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-财务会计',     false, true),
    ('q2c_cost_acct',  '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-成本会计',     false, true),
    ('q2c_cashier',    '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-出纳',         false, true),
    ('q2c_gl_acct',    '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-总账会计',     false, true),
    ('q2c_gm',         '$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI', 'Q2C-总经理',       false, true)
ON CONFLICT (username) DO NOTHING;

-- ============================================================
-- 5. 用户-角色关联
-- ============================================================

-- q2c_sales → q2c_sales_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_sales' AND r.role_code = 'q2c_sales_role'
ON CONFLICT DO NOTHING;

-- q2c_sales_mgr → q2c_sales_mgr_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_sales_mgr' AND r.role_code = 'q2c_sales_mgr_role'
ON CONFLICT DO NOTHING;

-- q2c_planner → q2c_planner_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_planner' AND r.role_code = 'q2c_planner_role'
ON CONFLICT DO NOTHING;

-- q2c_buyer → q2c_buyer_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_buyer' AND r.role_code = 'q2c_buyer_role'
ON CONFLICT DO NOTHING;

-- q2c_buyer_mgr → q2c_buyer_mgr_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_buyer_mgr' AND r.role_code = 'q2c_buyer_mgr_role'
ON CONFLICT DO NOTHING;

-- q2c_prod_mgr → q2c_prod_mgr_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_prod_mgr' AND r.role_code = 'q2c_prod_mgr_role'
ON CONFLICT DO NOTHING;

-- q2c_operator → q2c_operator_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_operator' AND r.role_code = 'q2c_operator_role'
ON CONFLICT DO NOTHING;

-- q2c_qc → q2c_qc_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_qc' AND r.role_code = 'q2c_qc_role'
ON CONFLICT DO NOTHING;

-- q2c_qc_mgr → q2c_qc_mgr_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_qc_mgr' AND r.role_code = 'q2c_qc_mgr_role'
ON CONFLICT DO NOTHING;

-- q2c_warehouse → q2c_warehouse_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_warehouse' AND r.role_code = 'q2c_warehouse_role'
ON CONFLICT DO NOTHING;

-- q2c_accountant → q2c_accountant_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_accountant' AND r.role_code = 'q2c_accountant_role'
ON CONFLICT DO NOTHING;

-- q2c_cost_acct → q2c_cost_acct_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_cost_acct' AND r.role_code = 'q2c_cost_acct_role'
ON CONFLICT DO NOTHING;

-- q2c_cashier → q2c_cashier_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_cashier' AND r.role_code = 'q2c_cashier_role'
ON CONFLICT DO NOTHING;

-- q2c_gl_acct → q2c_gl_acct_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_gl_acct' AND r.role_code = 'q2c_gl_acct_role'
ON CONFLICT DO NOTHING;

-- q2c_gm → q2c_gm_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'q2c_gm' AND r.role_code = 'q2c_gm_role'
ON CONFLICT DO NOTHING;

COMMIT;
