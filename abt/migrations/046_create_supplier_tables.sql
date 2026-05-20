CREATE TABLE suppliers (
    supplier_id    BIGSERIAL PRIMARY KEY,
    supplier_code  VARCHAR(50) NOT NULL UNIQUE,
    supplier_name  VARCHAR(200) NOT NULL,
    short_name     VARCHAR(100),
    classification VARCHAR(10) NOT NULL DEFAULT 'C',
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_suppliers_status ON suppliers(status) WHERE deleted_at IS NULL;

CREATE TABLE supplier_contacts (
    contact_id     BIGSERIAL PRIMARY KEY,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id) ON DELETE CASCADE,
    contact_name   VARCHAR(100) NOT NULL,
    phone          VARCHAR(50),
    email          VARCHAR(100),
    position       VARCHAR(100),
    is_primary     BOOLEAN NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_supplier_contacts_supplier ON supplier_contacts(supplier_id);

CREATE TABLE supplier_bank_accounts (
    bank_account_id BIGSERIAL PRIMARY KEY,
    supplier_id     BIGINT NOT NULL REFERENCES suppliers(supplier_id) ON DELETE CASCADE,
    bank_name       VARCHAR(200) NOT NULL,
    account_name    VARCHAR(200) NOT NULL,
    account_no      VARCHAR(100) NOT NULL,
    is_default      BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_supplier_bank_accounts_supplier ON supplier_bank_accounts(supplier_id);

CREATE TABLE supplier_prices (
    price_id       BIGSERIAL PRIMARY KEY,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id),
    product_id     BIGINT NOT NULL,
    unit_price     DECIMAL(14,6) NOT NULL,
    valid_from     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_until    TIMESTAMPTZ NOT NULL,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_supplier_prices_lookup
    ON supplier_prices(supplier_id, product_id, valid_until);
