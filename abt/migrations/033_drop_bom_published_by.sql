-- 删除 bom 表的 published_by 列（created_by 已记录创建者）
ALTER TABLE bom DROP COLUMN IF EXISTS published_by;
