-- 021_add_missing_indexes.sql
-- 补全 abt_v2 数据库中缺失的索引

-- ============================================================
-- 1. 关联表索引
-- ============================================================

-- 角色权限关联（结构: role_id, resource_code, action）
CREATE INDEX IF NOT EXISTS idx_role_permissions_role_id ON role_permissions (role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_resource_action ON role_permissions (resource_code, action);

-- 用户角色关联
CREATE INDEX IF NOT EXISTS idx_user_roles_user_id ON user_roles (user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role_id ON user_roles (role_id);

-- 用户部门关联
CREATE INDEX IF NOT EXISTS idx_user_departments_user_id ON user_departments (user_id);
CREATE INDEX IF NOT EXISTS idx_user_departments_department_id ON user_departments (department_id);

-- 产品分类关联
CREATE INDEX IF NOT EXISTS idx_product_categories_product_id ON product_categories (product_id);
CREATE INDEX IF NOT EXISTS idx_product_categories_category_id ON product_categories (category_id);

-- 拣货策略
CREATE INDEX IF NOT EXISTS idx_pick_strategies_warehouse_id ON pick_strategies (warehouse_id);

-- 上架策略
CREATE INDEX IF NOT EXISTS idx_putaway_strategies_warehouse_id ON putaway_strategies (warehouse_id);
CREATE INDEX IF NOT EXISTS idx_putaway_strategies_product_category_id ON putaway_strategies (product_category_id);

-- ============================================================
-- 2. status 字段索引
-- ============================================================

-- backflush_records / cycle_counts 无 deleted_at，不加 WHERE 子句
CREATE INDEX IF NOT EXISTS idx_backflush_records_status ON backflush_records (status);
CREATE INDEX IF NOT EXISTS idx_cycle_counts_status ON cycle_counts (status);
-- 以下表有 deleted_at，用部分索引
CREATE INDEX IF NOT EXISTS idx_bins_status ON bins (status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_form_conversions_status ON form_conversions (status);
CREATE INDEX IF NOT EXISTS idx_idempotency_records_status ON idempotency_records (status);
CREATE INDEX IF NOT EXISTS idx_inspection_results_status ON inspection_results (status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_inventory_locks_status ON inventory_locks (status);
CREATE INDEX IF NOT EXISTS idx_inventory_transfers_status ON inventory_transfers (status);
CREATE INDEX IF NOT EXISTS idx_production_plan_items_status ON production_plan_items (status);
CREATE INDEX IF NOT EXISTS idx_production_receipts_status ON production_receipts (status);
CREATE INDEX IF NOT EXISTS idx_task_run_logs_status ON task_run_logs (status);
CREATE INDEX IF NOT EXISTS idx_warehouses_status ON warehouses (status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_work_order_routings_status ON work_order_routings (status);

-- ============================================================
-- 3. deleted_at 软删除部分索引
-- ============================================================

CREATE INDEX IF NOT EXISTS idx_boms_active ON boms (bom_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_outsourcing_orders_active ON outsourcing_orders (id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_production_plans_active ON production_plans (id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_quotations_active ON quotations (id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_reconciliations_active ON reconciliations (id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_routings_active ON routings (id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_sales_orders_active ON sales_orders (id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_sales_returns_active ON sales_returns (id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_shipping_requests_active ON shipping_requests (id) WHERE deleted_at IS NULL;

-- ============================================================
-- 4. 明细表 product_id 索引
-- ============================================================

CREATE INDEX IF NOT EXISTS idx_ani_product ON arrival_notice_items (product_id);
CREATE INDEX IF NOT EXISTS idx_bi_product ON backflush_items (component_id);
CREATE INDEX IF NOT EXISTS idx_backflush_records_product ON backflush_records (product_id);
CREATE INDEX IF NOT EXISTS idx_ci_product ON conversion_items (product_id);
CREATE INDEX IF NOT EXISTS idx_cci_product ON cycle_count_items (product_id);
CREATE INDEX IF NOT EXISTS idx_inventory_locks_product ON inventory_locks (product_id);
CREATE INDEX IF NOT EXISTS idx_mri_product ON material_requisition_items (product_id);
CREATE INDEX IF NOT EXISTS idx_outsourcing_materials_product ON outsourcing_materials (product_id);
CREATE INDEX IF NOT EXISTS idx_outsourcing_orders_product ON outsourcing_orders (product_id);
CREATE INDEX IF NOT EXISTS idx_production_batches_product ON production_batches (product_id);
CREATE INDEX IF NOT EXISTS idx_production_receipts_product ON production_receipts (product_id);
CREATE INDEX IF NOT EXISTS idx_purchase_return_items_product ON purchase_return_items (product_id);
CREATE INDEX IF NOT EXISTS idx_quotation_items_product ON quotation_items (product_id);
CREATE INDEX IF NOT EXISTS idx_reconciliation_items_product ON reconciliation_items (product_id);
CREATE INDEX IF NOT EXISTS idx_sales_order_items_product ON sales_order_items (product_id);
CREATE INDEX IF NOT EXISTS idx_sales_return_items_product ON sales_return_items (product_id);
CREATE INDEX IF NOT EXISTS idx_shipping_request_items_product ON shipping_request_items (product_id);
CREATE INDEX IF NOT EXISTS idx_transfer_items_product ON transfer_items (product_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_product ON work_orders (product_id);

-- ============================================================
-- 5. 明细表父表 ID 索引（主从关联查询）
-- ============================================================

-- 到货通知
CREATE INDEX IF NOT EXISTS idx_ani_order_item ON arrival_notice_items (order_item_id);

-- 采购对账明细
CREATE INDEX IF NOT EXISTS idx_prci_order ON purchase_recon_items (order_id);
CREATE INDEX IF NOT EXISTS idx_prci_order_item ON purchase_recon_items (order_item_id);

-- 采购退货明细
CREATE INDEX IF NOT EXISTS idx_pri_order_item ON purchase_return_items (order_item_id);

-- 销售退货明细
CREATE INDEX IF NOT EXISTS idx_sri_order_item ON sales_return_items (order_item_id);

-- 发货请求明细
CREATE INDEX IF NOT EXISTS idx_sri_shipping_order_item ON shipping_request_items (order_item_id);

-- 库存事务 bin_id
CREATE INDEX IF NOT EXISTS idx_txn_bin ON inventory_transactions (bin_id);

-- 生产收货 bin/zone/warehouse
CREATE INDEX IF NOT EXISTS idx_production_receipts_bin ON production_receipts (bin_id);
CREATE INDEX IF NOT EXISTS idx_production_receipts_zone ON production_receipts (zone_id);
CREATE INDEX IF NOT EXISTS idx_production_receipts_warehouse ON production_receipts (warehouse_id);

-- 领料明细 bin
CREATE INDEX IF NOT EXISTS idx_mri_bin ON material_requisition_items (bin_id);

-- 盘点明细 bin
CREATE INDEX IF NOT EXISTS idx_cci_bin ON cycle_count_items (bin_id);

-- 库存调拨 from/to
CREATE INDEX IF NOT EXISTS idx_inventory_transfers_from_bin ON inventory_transfers (from_bin_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transfers_from_warehouse ON inventory_transfers (from_warehouse_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transfers_from_zone ON inventory_transfers (from_zone_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transfers_to_bin ON inventory_transfers (to_bin_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transfers_to_warehouse ON inventory_transfers (to_warehouse_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transfers_to_zone ON inventory_transfers (to_zone_id);

-- 库存预留 source_line
CREATE INDEX IF NOT EXISTS idx_inv_res_source_line ON inventory_reservations (source_line_id);

-- 到货通知 仓库/库位
CREATE INDEX IF NOT EXISTS idx_arrival_warehouse ON arrival_notices (warehouse_id);
CREATE INDEX IF NOT EXISTS idx_arrival_zone ON arrival_notices (zone_id);

-- 领料单 仓库
CREATE INDEX IF NOT EXISTS idx_material_requisitions_warehouse ON material_requisitions (warehouse_id);

-- 盘点 仓库/库位
CREATE INDEX IF NOT EXISTS idx_cycle_counts_warehouse ON cycle_counts (warehouse_id);
CREATE INDEX IF NOT EXISTS idx_cycle_counts_zone ON cycle_counts (zone_id);

-- 发货请求明细 仓库
CREATE INDEX IF NOT EXISTS idx_shipping_request_items_warehouse ON shipping_request_items (warehouse_id);

-- 生产计划明细
CREATE INDEX IF NOT EXISTS idx_plan_items_bom_snapshot ON production_plan_items (bom_snapshot_id);
CREATE INDEX IF NOT EXISTS idx_plan_items_routing ON production_plan_items (routing_id);
CREATE INDEX IF NOT EXISTS idx_plan_items_sales_order ON production_plan_items (sales_order_id);
CREATE INDEX IF NOT EXISTS idx_plan_items_sales_order_item ON production_plan_items (sales_order_item_id);
CREATE INDEX IF NOT EXISTS idx_plan_items_work_center ON production_plan_items (work_center_id);

-- 工单
CREATE INDEX IF NOT EXISTS idx_work_orders_bom_snapshot ON work_orders (bom_snapshot_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_routing ON work_orders (routing_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_sales_order ON work_orders (sales_order_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_work_center ON work_orders (work_center_id);

-- 工单工艺 work_center
CREATE INDEX IF NOT EXISTS idx_work_order_routings_work_center ON work_order_routings (work_center_id);

-- 库存锁定 仓库/客户
CREATE INDEX IF NOT EXISTS idx_inventory_locks_warehouse ON inventory_locks (warehouse_id);
CREATE INDEX IF NOT EXISTS idx_inventory_locks_customer ON inventory_locks (customer_id);

-- 对账明细
CREATE INDEX IF NOT EXISTS idx_reconciliation_items_sales_order ON reconciliation_items (sales_order_id);
CREATE INDEX IF NOT EXISTS idx_reconciliation_items_shipping_request ON reconciliation_items (shipping_request_id);

-- 采购订单明细 quotation_item
CREATE INDEX IF NOT EXISTS idx_poi_quotation_item ON purchase_order_items (quotation_item_id);

-- 采购报价明细 product
CREATE INDEX IF NOT EXISTS idx_pqi_product ON purchase_quotation_items (product_id);

-- 报工
CREATE INDEX IF NOT EXISTS idx_work_reports_routing ON work_reports (routing_id);

-- 生产检验 routing
CREATE INDEX IF NOT EXISTS idx_production_inspections_routing ON production_inspections (routing_id);

-- 委外订单
CREATE INDEX IF NOT EXISTS idx_outsourcing_orders_routing ON outsourcing_orders (routing_id);
CREATE INDEX IF NOT EXISTS idx_outsourcing_orders_virtual_warehouse ON outsourcing_orders (virtual_warehouse_id);

-- 付款请求
CREATE INDEX IF NOT EXISTS idx_payment_requests_reconciliation ON payment_requests (reconciliation_id);
CREATE INDEX IF NOT EXISTS idx_payment_requests_bank_account ON payment_requests (bank_account_id);

-- 报价单
CREATE INDEX IF NOT EXISTS idx_quotations_contact ON quotations (contact_id);
CREATE INDEX IF NOT EXISTS idx_quotations_sales_rep ON quotations (sales_rep_id);

-- 销售订单
CREATE INDEX IF NOT EXISTS idx_sales_orders_contact ON sales_orders (contact_id);
CREATE INDEX IF NOT EXISTS idx_sales_orders_sales_rep ON sales_orders (sales_rep_id);

-- 库存事务 仓库/库位
CREATE INDEX IF NOT EXISTS idx_txn_warehouse ON inventory_transactions (warehouse_id);
CREATE INDEX IF NOT EXISTS idx_txn_zone ON inventory_transactions (zone_id);

-- 角色 parent
CREATE INDEX IF NOT EXISTS idx_roles_parent ON roles (parent_role_id);

-- ============================================================
-- 6. 代码审查补充（基于实际 SQL 查询模式）
-- ============================================================

-- backflush_records: 代码按 work_order_id 过滤列表
CREATE INDEX IF NOT EXISTS idx_backflush_records_work_order ON backflush_records (work_order_id);

-- form_conversions: 代码按 warehouse_id 过滤列表
CREATE INDEX IF NOT EXISTS idx_form_conversions_warehouse ON form_conversions (warehouse_id);

-- idempotency_records: 定时清理过期记录
CREATE INDEX IF NOT EXISTS idx_idempotency_records_expires ON idempotency_records (expires_at) WHERE expires_at IS NOT NULL;
