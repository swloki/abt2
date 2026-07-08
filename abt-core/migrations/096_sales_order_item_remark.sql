-- 销售订单明细增加备注列（每行产品可填备注）
ALTER TABLE sales_order_items ADD COLUMN IF NOT EXISTS remark TEXT NOT NULL DEFAULT '';
