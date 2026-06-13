-- MES 三层状态联动 — 历史数据回填
-- 根据已有批次状态回填工单和计划行状态
-- 执行前务必备份数据库

BEGIN;

-- 1. 回填 WorkOrder 状态：有 InProgress 批次的工单 → InProduction (6)
UPDATE work_orders wo
SET status = 6, version = version + 1, updated_at = NOW()
WHERE wo.status = 3  -- Released
  AND wo.deleted_at IS NULL
  AND EXISTS (
    SELECT 1 FROM production_batches pb
    WHERE pb.work_order_id = wo.id
      AND pb.status IN (2, 3, 4)  -- InProgress, Suspended, PendingReceipt
  );

-- 2. 回填 WorkOrder 状态：所有批次终态的工单 → Closed (4)
UPDATE work_orders wo
SET status = 4, version = version + 1, updated_at = NOW()
WHERE wo.status IN (3, 6)  -- Released or InProduction
  AND wo.deleted_at IS NULL
  AND EXISTS (
    SELECT 1 FROM production_batches pb
    WHERE pb.work_order_id = wo.id
      AND pb.status = 5  -- Completed
  )
  AND NOT EXISTS (
    SELECT 1 FROM production_batches pb
    WHERE pb.work_order_id = wo.id
      AND pb.status NOT IN (5, 6)  -- 排除有未完成批次的工单
  );

-- 3. 回填 PlanItem 状态：InProduction (3)
UPDATE production_plan_items ppi
SET status = 3  -- InProduction
WHERE ppi.status = 2  -- Released
  AND EXISTS (
    SELECT 1 FROM work_orders wo
    WHERE wo.plan_item_id = ppi.id
      AND wo.status = 6  -- InProduction
  );

-- 4. 回填 PlanItem 状态：Completed (4)
UPDATE production_plan_items ppi
SET status = 4  -- Completed
WHERE ppi.status IN (2, 3)  -- Released or InProduction
  AND EXISTS (
    SELECT 1 FROM work_orders wo
    WHERE wo.plan_item_id = ppi.id
      AND wo.status = 4  -- Closed
  );

-- 5. 回填 Plan 状态：Completed (4)
UPDATE production_plans pp
SET status = 4, updated_at = NOW()
WHERE pp.status = 3  -- InProgress
  AND NOT EXISTS (
    SELECT 1 FROM production_plan_items ppi
    WHERE ppi.plan_id = pp.id
      AND ppi.status NOT IN (4, 5)  -- 非 Completed/Cancelled
  );

COMMIT;

-- 验证查询
SELECT 'work_orders' AS table_name, status, COUNT(*) FROM work_orders WHERE deleted_at IS NULL GROUP BY status ORDER BY status;
SELECT 'production_plan_items' AS table_name, status, COUNT(*) FROM production_plan_items GROUP BY status ORDER BY status;
SELECT 'production_plans' AS table_name, status, COUNT(*) FROM production_plans WHERE deleted_at IS NULL GROUP BY status ORDER BY status;
