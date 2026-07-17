-- 修复 work_order_routings.step_no 不从 1 开始的语义 bug（issue #260 真正根因）
-- =========================================================================
-- 根因：load_operations_from_bom 曾把 BOM 源 bom_operations.step_order 直接拷成
-- work_order_routings.step_no。但 step_no 语义是「工单工序序号（1-based 连续）」，
-- 矩阵 active_step / 报工防跳序 (current_step != step_no-1) / next_step(step_no+1)
-- 都依赖它。BOM step_order 不从 1 时（实测 244 工单首道=0、2 工单首道=5），
-- 矩阵全显示「—」、报工被防跳序拦截。
--
-- 本脚本：重编号受影响工单的 step_no 为 1-based + 同步 production_batches.current_step。
-- work_reports 用 routing_id 外键（不存 step_no），不受影响。
--
-- 幂等：第二次跑时受影响集为空（已全部 MIN(step_no)=1），无副作用。
-- 用法：psql "$DATABASE_URL" -f scripts/fix-work-order-routing-stepno.sql
-- =========================================================================

BEGIN;

-- 1. 建 old_step → new_step 映射（按 work_order 分组，step_no 升序 ROW_NUMBER）
CREATE TEMP TABLE step_remap ON COMMIT DROP AS
SELECT id AS routing_id,
       work_order_id,
       step_no AS old_step,
       ROW_NUMBER() OVER (PARTITION BY work_order_id ORDER BY step_no) AS new_step
FROM work_order_routings
WHERE work_order_id IN (
    SELECT work_order_id FROM work_order_routings
    GROUP BY work_order_id HAVING MIN(step_no) <> 1
);

-- 2a. 先偏移到负值临时区（避开 (work_order_id, step_no) UNIQUE 约束的逐行冲突）
UPDATE work_order_routings r
SET step_no = -m.new_step
FROM step_remap m
WHERE r.id = m.routing_id;

-- 2b. 翻为正值终值（1-based）
UPDATE work_order_routings
SET step_no = -step_no
WHERE step_no < 0;

-- 3. 同步批次 current_step（old→new）。
--    current_step=0 是「未开工」语义，不映射（AND m.old_step <> 0）：
--    step_no=0 工单的首道报不了工（防跳序 current_step != -1），current_step=0
--    一定是未开工初始值，保持 0。
UPDATE production_batches b
SET current_step = m.new_step
FROM step_remap m
WHERE b.work_order_id = m.work_order_id
  AND b.current_step = m.old_step
  AND m.old_step <> 0;

-- 4. 验证：所有工单首道 step_no 应为 1（期望 0 行）
SELECT work_order_id, MIN(step_no) AS first_step
FROM work_order_routings
GROUP BY work_order_id
HAVING MIN(step_no) <> 1;

COMMIT;
