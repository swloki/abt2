-- 初始化销售模块文档序列
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES
    ('SO', 'SO', 0, 'monthly'),
    ('SR', 'SR', 0, 'monthly'),
    ('RT', 'RT', 0, 'monthly'),
    ('RC', 'RC', 0, 'monthly')
ON CONFLICT (doc_type) DO NOTHING;
