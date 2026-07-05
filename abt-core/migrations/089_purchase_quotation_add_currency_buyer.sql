-- 089: 采购报价主表加币种 / 采购员 / 供应商报价单号
--   currency             — 报价币种（行级 currency 退化为冗余，create 时由主表继承）
--   buyer_id             — 负责采购员（区别于 operator_id 录入人；nullable）
--   supplier_quotation_no — 供应商自带报价单号（便于核对纸质件 / 邮件）
--
-- 老数据回填：采购员回填为录入人（详情页原本就按 operator_id 显示「采购员」，语义延续）。

ALTER TABLE purchase_quotations
    ADD COLUMN currency              VARCHAR(3)  NOT NULL DEFAULT 'CNY',
    ADD COLUMN buyer_id              BIGINT,
    ADD COLUMN supplier_quotation_no VARCHAR(64) NOT NULL DEFAULT '';

UPDATE purchase_quotations SET buyer_id = operator_id WHERE buyer_id IS NULL;
