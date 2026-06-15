-- ============================================================================
-- 045: Enrich routing_steps with operation attributes + requisition items
-- 参考: Odoo mrp.routing.workcenter (workcenter_id, time_cycle, cost_mode)
--       ERPNext Operation (workstation, quality_inspection_template)
--       ABT bom_labor_processes (unit_price — 中国制造业计件工资特色)
-- ============================================================================

BEGIN;

-- 1. routing_steps 加工序属性字段
--    原 routing_steps 只有 process_code/step_order/is_required/remark
--    工单下达(release)时无法将这些属性映射到 work_order_routings
--    导致计件单价/标准工时/检验点/委外标记全部丢失
ALTER TABLE routing_steps ADD COLUMN IF NOT EXISTS work_center_id BIGINT;
ALTER TABLE routing_steps ADD COLUMN IF NOT EXISTS standard_time DECIMAL(18,6);      -- 标准工时(分钟)，对标 Odoo time_cycle_manual
ALTER TABLE routing_steps ADD COLUMN IF NOT EXISTS standard_cost DECIMAL(18,6);       -- 标准成本(每小时)，对标 Odoo workcenter.costs_hour
ALTER TABLE routing_steps ADD COLUMN IF NOT EXISTS unit_price DECIMAL(18,6) DEFAULT 0; -- 计件单价，中国制造业特色
ALTER TABLE routing_steps ADD COLUMN IF NOT EXISTS allowed_loss_rate DECIMAL(18,6) DEFAULT 0; -- 允许损耗率
ALTER TABLE routing_steps ADD COLUMN IF NOT EXISTS is_outsourced BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE routing_steps ADD COLUMN IF NOT EXISTS is_inspection_point BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_routing_steps_work_center ON routing_steps (work_center_id);

-- 2. material_requisition_items 加工序/批次关联
--    对标 Odoo stock.move.operation_id（领料精确到工序）
--    支持按工序分阶段领料 + 按流转卡(batch)追踪物料
ALTER TABLE material_requisition_items ADD COLUMN IF NOT EXISTS operation_id BIGINT; -- 关联 work_order_routings.id
ALTER TABLE material_requisition_items ADD COLUMN IF NOT EXISTS batch_id BIGINT;      -- 关联 production_batches.id

CREATE INDEX IF NOT EXISTS idx_mri_operation ON material_requisition_items (operation_id);
CREATE INDEX IF NOT EXISTS idx_mri_batch ON material_requisition_items (batch_id);

COMMIT;
