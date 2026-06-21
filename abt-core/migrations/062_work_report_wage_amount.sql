-- 报工冻结工资：报工落库时写入 wage_amount，避免后续改工序单价导致历史工资漂移。
-- （调研③的实质：报工即冻结费率）
ALTER TABLE work_reports ADD COLUMN IF NOT EXISTS wage_amount NUMERIC(20,4) NOT NULL DEFAULT 0;

-- 回填历史报工（与运行时公式一致）：
--   wage = (completed_qty + affect_wage 的不良量) × unit_price
--   affect_wage=true 的 DefectReason：1=MaterialDefect, 2=EquipmentFault, 4=ProcessIssue
--   （3=OperatorError 不影响工资）
UPDATE work_reports wr
SET wage_amount = (wr.completed_qty +
        CASE WHEN wr.defect_reason IN (1, 2, 4) THEN wr.defect_qty ELSE 0 END)
    * COALESCE((SELECT wor.unit_price FROM work_order_routings wor WHERE wor.id = wr.routing_id), 0)
WHERE wr.wage_amount = 0;
