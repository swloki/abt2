BEGIN;

-- 销售订单编号序列
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES ('SO', 'SO-', 0, 'monthly');

-- 发货申请编号序列
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES ('SR', 'SR-', 0, 'monthly');

-- 退货单编号序列
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES ('RT', 'RT-', 0, 'monthly');

-- 对账单编号序列
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES ('RC', 'RC-', 0, 'monthly');

COMMIT;
