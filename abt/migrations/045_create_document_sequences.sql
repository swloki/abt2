-- Document sequence generator for business documents (quotations, orders, etc.)
CREATE TABLE document_sequences (
    sequence_id   BIGSERIAL PRIMARY KEY,
    doc_type      VARCHAR(20) NOT NULL UNIQUE,
    prefix        VARCHAR(10) NOT NULL,
    current_value INT NOT NULL DEFAULT 0,
    reset_rule    VARCHAR(10) NOT NULL DEFAULT 'monthly',
    last_reset_at DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed quotation sequence
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES ('QT', 'QT', 0, 'monthly');
