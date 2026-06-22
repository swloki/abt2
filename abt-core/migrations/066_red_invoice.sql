-- 红字发票（退货/贷项通知单）支持
-- is_return=TRUE 表示红字发票，return_against 指向被冲销的原发票

ALTER TABLE sales_invoices ADD COLUMN IF NOT EXISTS is_return     BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE sales_invoices ADD COLUMN IF NOT EXISTS return_against BIGINT;

ALTER TABLE purchase_invoices ADD COLUMN IF NOT EXISTS is_return     BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE purchase_invoices ADD COLUMN IF NOT EXISTS return_against BIGINT;

COMMENT ON COLUMN sales_invoices.is_return IS '是否红字发票（退货冲销）';
COMMENT ON COLUMN sales_invoices.return_against IS '被冲销的原发票ID';
COMMENT ON COLUMN purchase_invoices.is_return IS '是否红字发票（退货冲销）';
COMMENT ON COLUMN purchase_invoices.return_against IS '被冲销的原发票ID';
