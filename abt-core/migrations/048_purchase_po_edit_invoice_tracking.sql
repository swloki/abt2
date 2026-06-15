BEGIN;

-- ============================================================================
-- 1. PO 明细增加已开票数量 + 行级发票状态
-- ============================================================================

ALTER TABLE purchase_order_items
    ADD COLUMN qty_invoiced   NUMERIC(18,6) NOT NULL DEFAULT 0,
    ADD COLUMN invoice_status SMALLINT     NOT NULL DEFAULT 1;
    -- 1=NoInvoice, 2=ToInvoice, 3=FullyInvoiced

-- ============================================================================
-- 2. PO 主表增加头级发票状态 + 开票百分比
-- ============================================================================

ALTER TABLE purchase_orders
    ADD COLUMN invoice_status SMALLINT   NOT NULL DEFAULT 1,
    ADD COLUMN per_billed     NUMERIC(5,2) NOT NULL DEFAULT 0;

COMMIT;
