-- ============================================================================
-- 040: work_order_routings 删除执行进度字段
--
-- ⚠️ 必须在代码更新 + cargo clippy 通过后执行
-- 执行进度已迁移到 batch_routing_progress（迁移 039）
-- ============================================================================

ALTER TABLE work_order_routings DROP COLUMN IF EXISTS completed_qty;
ALTER TABLE work_order_routings DROP COLUMN IF EXISTS defect_qty;
ALTER TABLE work_order_routings DROP COLUMN IF EXISTS status;

DROP INDEX IF EXISTS idx_work_order_routings_status;
