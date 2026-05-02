-- 回滚：恢复 published_by 列
ALTER TABLE bom ADD COLUMN published_by bigint;
