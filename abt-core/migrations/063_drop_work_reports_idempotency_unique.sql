-- 移除 work_reports 的幂等唯一约束 (batch_id, routing_id, worker_id, shift, report_date)
--
-- 该约束阻止「同工人同班次同天」分批报工（如上午 500 件 + 下午 200 件），
-- 与车间实际及三家 ERP 实践不符（Odoo workorder.time_ids / ERPNext job_card.time_logs /
-- OFBiz TimeEntry 均允许同工人多次报工），导致补报被静默丢弃（toast 成功但进度不累加）。
--
-- 防重复提交改由前端（提交后关闭 modal）+ 后端事务（一次提交一事务）保证。
-- 保留 doc_number UNIQUE（报工单号唯一）。
--
-- 用 DO 块动态定位约束名：PG 自动命名会按 63 字符截断（report_date → report_dat），
-- 硬编码约束名不可靠，按约束定义匹配最稳。

DO $$
DECLARE c text;
BEGIN
    SELECT conname INTO c FROM pg_constraint
    WHERE conrelid = 'work_reports'::regclass
      AND contype = 'u'
      AND pg_get_constraintdef(oid) LIKE '%batch_id, routing_id, worker_id, shift, report_date%';
    IF c IS NOT NULL THEN
        EXECUTE format('ALTER TABLE work_reports DROP CONSTRAINT %I', c);
    END IF;
END $$;
