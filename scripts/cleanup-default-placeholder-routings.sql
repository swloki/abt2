-- 清理历史「生产」虚拟默认占位工序
--
-- 背景：旧版 WorkOrderService::release 在工单无工序时会自动插入一条
-- process_name='生产' 的占位工序（产出品 product_id / 工作中心 / 计件单价全空）。
-- 改造后 release 不再自动插入工序（改由用户在下达 drawer 手动从 Routing 加载，
-- 且 release 校验工序非空），但此前已 release 的历史工单仍残留这条无用占位。
--
-- 本脚本仅清理「同时满足以下条件」的占位行，保守避免误删真实工序：
--   1. process_name = '生产'
--   2. product_id IS NULL          （真实工序必有产出品）
--   3. work_center_id IS NULL
--   4. unit_price IS NULL
--   5. 该工序无任何报工记录（已被报工的占位保留，以免破坏报工历史）
--   6. 该工单的工序仅此一道（避免误删多步工序中的某一步）
--
-- ⚠ 先跑 PREVIEW 核对待删行，确认后再取消注释执行 DELETE；建议先备份 work_order_routings。
-- 用法：psql "$DATABASE_URL" -f scripts/cleanup-default-placeholder-routings.sql

-- ═══ PREVIEW：将被清理的占位工序 ═══
SELECT wor.id, wor.work_order_id, wor.step_no, wor.process_name,
       wor.product_id, wor.work_center_id, wor.unit_price,
       wo.doc_number, wo.status
FROM work_order_routings wor
JOIN work_orders wo ON wo.id = wor.work_order_id
WHERE wor.process_name = '生产'
  AND wor.product_id IS NULL
  AND wor.work_center_id IS NULL
  AND wor.unit_price IS NULL
  AND NOT EXISTS (SELECT 1 FROM work_reports wr WHERE wr.routing_id = wor.id)
  AND wor.work_order_id IN (
      SELECT work_order_id
      FROM work_order_routings
      GROUP BY work_order_id
      HAVING COUNT(*) = 1
  )
ORDER BY wor.work_order_id;

-- ═══ DELETE：确认 PREVIEW 结果后，取消下面注释执行 ═══
-- DELETE FROM work_order_routings
-- WHERE process_name = '生产'
--   AND product_id IS NULL
--   AND work_center_id IS NULL
--   AND unit_price IS NULL
--   AND NOT EXISTS (SELECT 1 FROM work_reports wr WHERE wr.routing_id = work_order_routings.id)
--   AND work_order_id IN (
--       SELECT work_order_id
--       FROM work_order_routings
--       GROUP BY work_order_id
--       HAVING COUNT(*) = 1
--   );
