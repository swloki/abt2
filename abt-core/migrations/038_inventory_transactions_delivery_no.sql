-- 038: inventory_transactions 增加 delivery_no（送货单号）列
ALTER TABLE inventory_transactions ADD COLUMN IF NOT EXISTS delivery_no VARCHAR(100);
