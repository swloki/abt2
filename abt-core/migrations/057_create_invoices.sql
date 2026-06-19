-- 销售发票（AR）+ 采购发票（AP）表 + 状态机 seed
-- 含 gl_entry_id 列供 B2/B3 发票 cancel 时同步 cancel GL 凭证

-- ── 销售发票表 ──
CREATE TABLE sales_invoices (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(40) NOT NULL,
    customer_id     BIGINT      NOT NULL,
    issue_date      DATE        NOT NULL,
    period          VARCHAR(20) NOT NULL,
    subtotal        DECIMAL(18,6) NOT NULL DEFAULT 0,
    tax_amount      DECIMAL(18,6) NOT NULL DEFAULT 0,
    total           DECIMAL(18,6) NOT NULL DEFAULT 0,
    status          SMALLINT    NOT NULL DEFAULT 1,  -- 1draft/2posted/3cancelled
    source_shipping_id BIGINT,
    gl_entry_id     BIGINT,                              -- GL 凭证 ID（cancel 时同步 cancel）
    operator_id     BIGINT      NOT NULL,
    version         INTEGER     NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);
CREATE INDEX idx_sales_invoices_customer ON sales_invoices(customer_id);
CREATE INDEX idx_sales_invoices_shipping ON sales_invoices(source_shipping_id);
CREATE INDEX idx_sales_invoices_gl_entry ON sales_invoices(gl_entry_id);

-- ── 销售发票行 ──
CREATE TABLE sales_invoice_items (
    id              BIGSERIAL   PRIMARY KEY,
    invoice_id      BIGINT      NOT NULL REFERENCES sales_invoices(id),
    product_id      BIGINT      NOT NULL,
    qty             DECIMAL(18,6) NOT NULL,
    unit_price      DECIMAL(18,6) NOT NULL,
    tax_rate_id     BIGINT,
    line_subtotal  DECIMAL(18,6) NOT NULL,
    line_tax        DECIMAL(18,6) NOT NULL,
    line_total      DECIMAL(18,6) NOT NULL
);
CREATE INDEX idx_sales_invoice_items_invoice ON sales_invoice_items(invoice_id);
CREATE INDEX idx_sales_invoice_items_product ON sales_invoice_items(product_id);

-- ── 采购发票表（同构）──
CREATE TABLE purchase_invoices (
    id              BIGSERIAL   PRIMARY KEY,
    doc_number      VARCHAR(40) NOT NULL,
    supplier_id     BIGINT      NOT NULL,
    issue_date      DATE        NOT NULL,
    period          VARCHAR(20) NOT NULL,
    subtotal        DECIMAL(18,6) NOT NULL DEFAULT 0,
    tax_amount      DECIMAL(18,6) NOT NULL DEFAULT 0,
    total           DECIMAL(18,6) NOT NULL DEFAULT 0,
    status          SMALLINT    NOT NULL DEFAULT 1,  -- 1draft/2posted/3cancelled
    source_arrival_id BIGINT,
    gl_entry_id     BIGINT,                              -- GL 凭证 ID（cancel 时同步 cancel）
    operator_id     BIGINT      NOT NULL,
    version         INTEGER     NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);
CREATE INDEX idx_purchase_invoices_supplier ON purchase_invoices(supplier_id);
CREATE INDEX idx_purchase_invoices_arrival ON purchase_invoices(source_arrival_id);
CREATE INDEX idx_purchase_invoices_gl_entry ON purchase_invoices(gl_entry_id);

-- ── 采购发票行（同构）──
CREATE TABLE purchase_invoice_items (
    id              BIGSERIAL   PRIMARY KEY,
    invoice_id      BIGINT      NOT NULL REFERENCES purchase_invoices(id),
    product_id      BIGINT      NOT NULL,
    qty             DECIMAL(18,6) NOT NULL,
    unit_price      DECIMAL(18,6) NOT NULL,
    tax_rate_id     BIGINT,
    line_subtotal  DECIMAL(18,6) NOT NULL,
    line_tax        DECIMAL(18,6) NOT NULL,
    line_total      DECIMAL(18,6) NOT NULL
);
CREATE INDEX idx_purchase_invoice_items_invoice ON purchase_invoice_items(invoice_id);
CREATE INDEX idx_purchase_invoice_items_product ON purchase_invoice_items(product_id);

-- ── 状态机 seed ──
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('SalesInvoiceStatus', 'Draft',     '草稿',    TRUE,  FALSE),
    ('SalesInvoiceStatus', 'Posted',    '已过账',  FALSE, FALSE),
    ('SalesInvoiceStatus', 'Cancelled', '已取消',  FALSE, TRUE),
    ('PurchaseInvoiceStatus', 'Draft',     '草稿',    TRUE,  FALSE),
    ('PurchaseInvoiceStatus', 'Posted',    '已过账',  FALSE, FALSE),
    ('PurchaseInvoiceStatus', 'Cancelled', '已取消',  FALSE, TRUE)
ON CONFLICT (entity_type, state_name) DO NOTHING;

INSERT INTO state_transition_defs (entity_type, from_state, to_state, sort_order) VALUES
    ('SalesInvoiceStatus', '',          'Draft',     1),
    ('SalesInvoiceStatus', 'Draft',     'Posted',    2),
    ('SalesInvoiceStatus', 'Draft',     'Cancelled', 3),
    ('PurchaseInvoiceStatus', '',          'Draft',     1),
    ('PurchaseInvoiceStatus', 'Draft',     'Posted',    2),
    ('PurchaseInvoiceStatus', 'Draft',     'Cancelled', 3)
ON CONFLICT (entity_type, from_state, to_state) DO NOTHING;
