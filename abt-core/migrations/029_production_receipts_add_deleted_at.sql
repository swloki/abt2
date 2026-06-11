-- 生产入库单添加软删除字段（与其他业务表保持一致）
ALTER TABLE production_receipts ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMP WITH TIME ZONE;
