-- 通用附件系统（attachments 表，migration 103）已替代报价单专用的 quotation_attachments。
-- 报价附件迁移到通用 attachments（owner_type='quotation'），删除专用表。
DROP TABLE IF EXISTS quotation_attachments;
