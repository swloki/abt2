-- Print Templates: WYSIWYG template editor for printing documents
CREATE TABLE IF NOT EXISTS print_templates (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    document_type VARCHAR(50) NOT NULL,
    description TEXT,
    html_content TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

-- One default per document_type
CREATE UNIQUE INDEX IF NOT EXISTS idx_print_templates_default_unique
    ON print_templates (document_type, is_default)
    WHERE is_default = TRUE AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_print_templates_document_type
    ON print_templates (document_type) WHERE deleted_at IS NULL;
