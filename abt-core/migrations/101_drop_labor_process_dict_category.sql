-- 101: 退役 labor_process_dicts.process_category（类别字段过度智能化，改回产出品可选 + 检验点/委外独立勾选，与三家 ERP 一致）
-- 该列由 100 引入，现有数据全 NULL，DROP 安全。保留 default_work_center_id / default_standard_time。
ALTER TABLE labor_process_dicts DROP COLUMN IF EXISTS process_category;
