CREATE TABLE document_sequences (
    sequence_id   BIGSERIAL PRIMARY KEY,
    doc_type      VARCHAR(20) NOT NULL UNIQUE,
    prefix        VARCHAR(10) NOT NULL,
    current_value INTEGER NOT NULL DEFAULT 0,
    reset_rule    VARCHAR(20) NOT NULL DEFAULT 'monthly',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule) VALUES
('PO', 'PO-', 0, 'monthly'),
('PS', 'PS-', 0, 'monthly'),
('PP', 'PP-', 0, 'monthly')
ON CONFLICT (doc_type) DO NOTHING;
