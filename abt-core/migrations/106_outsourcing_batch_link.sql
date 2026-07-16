-- 106: outsourcing_orders 加 batch_id —— 工序委外 drawer 精确回写批次进度
--
-- 背景：工序级委外（OutsourcingType::Process）从 batch drawer 就地创建委外单时，
-- 需记录所属 production_batch，供 OutsourcingReceived EventHandler 精确回写
-- batch_routing_progress + 推进 current_step（而非按工单+工序猜测批次）。
ALTER TABLE outsourcing_orders ADD COLUMN IF NOT EXISTS batch_id BIGINT;

-- 活跃委外单（非取消/非转自制）按 工单+工序 查询索引：drawer 动作位判定 + 防重复建单
CREATE INDEX IF NOT EXISTS idx_outrouting_active
  ON outsourcing_orders (work_order_id, routing_id)
  WHERE deleted_at IS NULL AND status NOT IN (8, 7);  -- 8=Cancelled, 7=ConvertedToInternal
