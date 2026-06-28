-- 阶段4 迁移工具：把每个产品「最近一张已配工序产出品的工单」的工序成本字段
-- （产出品 product_id / 计件单价 unit_price / 工作中心 work_center_id / 标准工时 standard_time / 委外 is_outsourced）
-- 回填到该产品 routing 模板的 routing_steps。
--
-- 背景：routing_create 的 empty_as_none bug 导致 routing_steps 的 product_id/unit_price 等
-- 从未保存成功（BOM 人工成本因此长期为 0）。此脚本把工单层（work_order_routings）已有的配置
-- 反向同步到 routing_steps，让 BOM 人工成本（读 routing 模板）能读到。
--
-- 匹配键：bom_routings.routing_id ↔ routing_steps.routing_id；work_order_routings.step_no ↔ routing_steps.step_order
-- 源工单：每个产品最近一张、且工序有产出品的工单（LATERAL + EXISTS）
-- 安全：COALESCE 保留 routing_steps 原值（源字段为空时不覆盖）；仅 UPDATE，不删行
--
-- ⚠ 跑前务必备份：pg_dump --table=routing_steps ... 或手动记录。
-- 建议先注释掉 UPDATE 块、放开下方 SELECT 预览受影响行，确认后再跑 UPDATE。
-- 跑法：psql "$DATABASE_URL" -f scripts/backfill-routing-labor-from-work-order.sql

BEGIN;

-- ── 预览（默认启用；确认无误后注释掉这段、放开下面 UPDATE）──
SELECT rs.id, rs.routing_id, rs.step_order,
       rs.product_id     AS old_pid,    src.product_id     AS new_pid,
       rs.unit_price     AS old_price,  src.unit_price     AS new_price,
       rs.work_center_id AS old_wc,     src.work_center_id AS new_wc
FROM routing_steps rs
JOIN (
    SELECT br.routing_id,
           wor.step_no,
           wor.product_id,
           wor.unit_price,
           wor.work_center_id,
           wor.standard_time,
           wor.is_outsourced
    FROM bom_routings br
    JOIN products p ON p.product_code = br.product_code
    JOIN LATERAL (
        SELECT wo.id
        FROM work_orders wo
        WHERE wo.product_id = p.product_id
          AND EXISTS (
            SELECT 1 FROM work_order_routings r
            WHERE r.work_order_id = wo.id AND r.product_id IS NOT NULL
          )
        ORDER BY wo.created_at DESC
        LIMIT 1
    ) recent_wo ON true
    JOIN work_order_routings wor ON wor.work_order_id = recent_wo.id
    WHERE wor.product_id IS NOT NULL
) src ON rs.routing_id = src.routing_id AND rs.step_order = src.step_no;

-- ── 回填（预览确认后注释掉上面 SELECT、放开下面 UPDATE）──
-- UPDATE routing_steps rs
-- SET
--     product_id     = COALESCE(src.product_id,     rs.product_id),
--     unit_price     = COALESCE(src.unit_price,     rs.unit_price),
--     work_center_id = COALESCE(src.work_center_id, rs.work_center_id),
--     standard_time  = COALESCE(src.standard_time,  rs.standard_time),
--     is_outsourced  = src.is_outsourced
-- FROM (
--     SELECT br.routing_id,
--            wor.step_no,
--            wor.product_id,
--            wor.unit_price,
--            wor.work_center_id,
--            wor.standard_time,
--            wor.is_outsourced
--     FROM bom_routings br
--     JOIN products p ON p.product_code = br.product_code
--     JOIN LATERAL (
--         SELECT wo.id
--         FROM work_orders wo
--         WHERE wo.product_id = p.product_id
--           AND EXISTS (
--             SELECT 1 FROM work_order_routings r
--             WHERE r.work_order_id = wo.id AND r.product_id IS NOT NULL
--           )
--         ORDER BY wo.created_at DESC
--         LIMIT 1
--     ) recent_wo ON true
--     JOIN work_order_routings wor ON wor.work_order_id = recent_wo.id
--     WHERE wor.product_id IS NOT NULL
-- ) src
-- WHERE rs.routing_id = src.routing_id
--   AND rs.step_order = src.step_no;

COMMIT;
