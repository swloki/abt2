-- ============================================================================
-- 039: batch_routing_progress — 批次工序执行进度（写真相源）
--
-- 三层工序模型落地：
--   Layer 1: master_data.routings + routing_steps        (工序模板)
--   Layer 2: work_order_routings                          (工单工序快照+参数)
--   Layer 3: batch_routing_progress                       (批次执行进度)  ← 本迁移
--
-- 报工事务 confirm_routing_step 的累加目标从 work_order_routings 迁移到本表。
-- 同时为 work_orders / production_batches 补完成量字段，并从 work_reports 回填历史数据。
-- ============================================================================

-- 1. 批次工序执行进度表
CREATE TABLE IF NOT EXISTS batch_routing_progress (
    id              BIGSERIAL   PRIMARY KEY,
    batch_id        BIGINT      NOT NULL REFERENCES production_batches(id),
    routing_id      BIGINT      NOT NULL REFERENCES work_order_routings(id),
    status          SMALLINT    NOT NULL DEFAULT 1,   -- 1=Pending, 2=InProgress, 3=Completed, 4=Skipped
    completed_qty   DECIMAL(18,6) NOT NULL DEFAULT 0,
    defect_qty      DECIMAL(18,6) NOT NULL DEFAULT 0,
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (batch_id, routing_id)
);

CREATE INDEX IF NOT EXISTS idx_brp_batch   ON batch_routing_progress (batch_id);
CREATE INDEX IF NOT EXISTS idx_brp_routing ON batch_routing_progress (routing_id);

-- 2. work_orders 补完成量/报废量（冗余字段，报工事务内同步累加，供列表筛选）
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS completed_qty DECIMAL(18,6) NOT NULL DEFAULT 0;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS scrap_qty     DECIMAL(18,6) NOT NULL DEFAULT 0;

-- 3. production_batches 补 deleted_at（cancel 软删需要，若已有则跳过）
ALTER TABLE production_batches ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_batches_work_order_alive
    ON production_batches (work_order_id) WHERE deleted_at IS NULL;

-- 4. 从 work_reports 回填 batch_routing_progress（work_reports 是报工原子记录，最可靠）
--    每个 (batch_id, routing_id) 组合一条记录，SUM 各报工的数量
INSERT INTO batch_routing_progress (batch_id, routing_id, status, completed_qty, defect_qty, started_at)
SELECT
    wr.batch_id,
    wr.routing_id,
    CASE
        WHEN SUM(wr.completed_qty) + COALESCE(SUM(wr.defect_qty), 0) > 0 THEN 2  -- InProgress
        ELSE 1                                                                    -- Pending
    END,
    COALESCE(SUM(wr.completed_qty), 0),
    COALESCE(SUM(wr.defect_qty), 0),
    MIN(wr.created_at)
FROM work_reports wr
GROUP BY wr.batch_id, wr.routing_id
ON CONFLICT (batch_id, routing_id) DO NOTHING;

-- 5. 同步批次维度的工序状态：若批次 current_step 已到末步且状态 >= PendingReceipt(4)，
--    将该批次所有 batch_routing_progress 标记为 Completed(3)
UPDATE batch_routing_progress brp
SET status = 3,  -- Completed
    completed_at = pb.actual_end
FROM production_batches pb
WHERE brp.batch_id = pb.id
  AND pb.status >= 4        -- PendingReceipt / Completed
  AND brp.status < 3;

-- 6. 回填 production_batches.completed_qty / scrap_qty（从 batch_routing_progress SUM）
UPDATE production_batches pb
SET completed_qty = COALESCE((
        SELECT SUM(brp.completed_qty) FROM batch_routing_progress brp WHERE brp.batch_id = pb.id
    ), 0),
    scrap_qty = COALESCE((
        SELECT SUM(brp.defect_qty) FROM batch_routing_progress brp WHERE brp.batch_id = pb.id
    ), 0);

-- 7. 回填 work_orders.completed_qty / scrap_qty（从 production_batches SUM）
UPDATE work_orders wo
SET completed_qty = COALESCE((
        SELECT SUM(pb.completed_qty)
        FROM production_batches pb
        WHERE pb.work_order_id = wo.id AND pb.deleted_at IS NULL
    ), 0),
    scrap_qty = COALESCE((
        SELECT SUM(pb.scrap_qty)
        FROM production_batches pb
        WHERE pb.work_order_id = wo.id AND pb.deleted_at IS NULL
    ), 0);
