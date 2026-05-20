BEGIN;

CREATE TABLE document_sequences (
    sequence_id   BIGSERIAL PRIMARY KEY,
    doc_type      VARCHAR(20) NOT NULL UNIQUE,
    prefix        VARCHAR(10) NOT NULL,
    current_value INTEGER NOT NULL DEFAULT 0,
    reset_rule    VARCHAR(20) NOT NULL DEFAULT 'monthly',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Initialize quotation sequence
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES ('QT', 'QT-', 0, 'monthly');

COMMENT ON TABLE document_sequences IS '文档编号序列表，为各模块提供统一单据编号生成';
COMMENT ON COLUMN document_sequences.doc_type IS '文档类型标识（QT=报价单, SO=订单, PO=采购单等）';
COMMENT ON COLUMN document_sequences.reset_rule IS '序号重置规则：monthly=月度重置, yearly=年度重置, none=不重置';

COMMIT;
