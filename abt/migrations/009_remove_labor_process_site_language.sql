-- 移除 bom_labor_process 表中的 site_id 和 language_id 字段
-- 这些字段在当前设计中不需要，人工工序通过 product_code 直接关联产品

ALTER TABLE bom_labor_process DROP COLUMN IF EXISTS site_id;
ALTER TABLE bom_labor_process DROP COLUMN IF EXISTS language_id;

-- 清理相关索引
DROP INDEX IF EXISTS idx_bom_labor_process_site_lang;
