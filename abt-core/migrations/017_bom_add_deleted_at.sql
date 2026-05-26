-- 给 boms 表添加 deleted_at 软删除列
ALTER TABLE boms ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
