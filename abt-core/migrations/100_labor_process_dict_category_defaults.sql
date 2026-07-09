-- 100: 工序字典加「类别 + 默认工作中心 + 默认标准工时」
-- 用于 BOM 工序编辑页「选工序智能联动」：
--   选某工序时自动带出默认工作中心/工时；按类别驱动行内字段
--   （inspection→无产出+勾检验点；outsourcing→勾委外）。
-- 参考三家 ERP：工序主数据携带默认工作中心/工时（ERPNext Operation.total_operation_time/workstation）。

ALTER TABLE labor_process_dicts
    ADD COLUMN IF NOT EXISTS process_category VARCHAR(20),
    ADD COLUMN IF NOT EXISTS default_work_center_id BIGINT,
    ADD COLUMN IF NOT EXISTS default_standard_time DECIMAL(10,6);

COMMENT ON COLUMN labor_process_dicts.process_category IS '工序类别：machining加工 / inspection检验 / outsourcing外协 / other其他';
COMMENT ON COLUMN labor_process_dicts.default_work_center_id IS '默认工作中心（选该工序时自动带出；引用 work_centers.id，应用层校验，不加 FK）';
COMMENT ON COLUMN labor_process_dicts.default_standard_time IS '默认标准工时（分钟，选该工序时自动带出）';
