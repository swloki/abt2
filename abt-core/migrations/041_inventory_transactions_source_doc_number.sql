-- 041: inventory_transactions 增加 source_doc_number（来源单号）列
-- 用于存储来源单据的单号（如采购单号 PO-xxx、来料通知单号 AN-xxx），
-- 区分于本事务自身生成的入库单号 doc_number（RK 开头）。
ALTER TABLE inventory_transactions ADD COLUMN IF NOT EXISTS source_doc_number VARCHAR(100);
