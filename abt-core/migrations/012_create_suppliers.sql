-- ============================================================================
-- Suppliers — 供应商 (主表、联系人、银行账户)
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Suppliers — 供应商主表
-- ============================================================================

CREATE TABLE suppliers (
    supplier_id     BIGSERIAL   PRIMARY KEY,
    supplier_code   VARCHAR(100) NOT NULL,
    supplier_name   VARCHAR(255) NOT NULL,
    short_name      VARCHAR(100),
    category        SMALLINT    NOT NULL,           -- 1=RawMaterial, 2=Packaging, 3=Outsourcing, 4=Consumable, 5=Service
    status          SMALLINT    NOT NULL DEFAULT 1, -- 1=Prospective, 2=Qualified, 3=Probation, 4=Disqualified, 5=Blacklisted
    tax_number      VARCHAR(50),
    lead_time_days  INT         NOT NULL DEFAULT 0,
    payment_terms   TEXT,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_suppliers_supplier_code ON suppliers (supplier_code) WHERE deleted_at IS NULL;
CREATE INDEX idx_suppliers_status ON suppliers (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_suppliers_category ON suppliers (category) WHERE deleted_at IS NULL;
CREATE INDEX idx_suppliers_name_trgm ON suppliers USING gin (supplier_name gin_trgm_ops);

-- ============================================================================
-- 2. Supplier Contacts — 供应商联系人
-- ============================================================================

CREATE TABLE supplier_contacts (
    contact_id   BIGSERIAL   PRIMARY KEY,
    supplier_id  BIGINT      NOT NULL,
    contact_name VARCHAR(100) NOT NULL,
    position     VARCHAR(100),
    phone        VARCHAR(50),
    email        VARCHAR(100),
    is_primary   BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_supplier_contacts_supplier_id ON supplier_contacts (supplier_id);

-- ============================================================================
-- 3. Supplier Bank Accounts — 供应商银行账户
-- ============================================================================

CREATE TABLE supplier_bank_accounts (
    account_id     BIGSERIAL   PRIMARY KEY,
    supplier_id    BIGINT      NOT NULL,
    bank_name      VARCHAR(100) NOT NULL,
    account_name   VARCHAR(100) NOT NULL,
    account_number VARCHAR(50)  NOT NULL,
    is_default     BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_supplier_bank_accounts_supplier_id ON supplier_bank_accounts (supplier_id);

COMMIT;
