-- Rollback Migration 021: Labor Process Redesign
-- Reverts the three-layer labor process model and restores the old table

BEGIN;

-- 1. 恢复旧表名
ALTER TABLE IF EXISTS bom_labor_process_archived RENAME TO bom_labor_process;

-- 2. 删除新表（按依赖顺序）
DROP TABLE IF EXISTS bom_labor_cost;
DROP TABLE IF EXISTS labor_process_group_member;
DROP TABLE IF EXISTS labor_process_group;
DROP TABLE IF EXISTS labor_process;

-- 3. 移除 bom 表新增的列
ALTER TABLE bom DROP COLUMN IF EXISTS process_group_id;

COMMIT;
