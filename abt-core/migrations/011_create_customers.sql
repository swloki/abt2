-- ============================================================================
-- Customers — 客户 (主表、联系人、地址)
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Customers — 客户主表
-- ============================================================================

CREATE TABLE customers (
    customer_id       BIGSERIAL   PRIMARY KEY,
    customer_code     VARCHAR(100) NOT NULL,
    customer_name     VARCHAR(255) NOT NULL,
    short_name        VARCHAR(100),
    category          SMALLINT    NOT NULL,           -- 1=Distributor, 2=DirectCustomer, 3=OEM, 4=Retailer
    status            SMALLINT    NOT NULL DEFAULT 1, -- 1=Prospective, 2=Active, 3=Inactive, 4=Blacklisted
    tax_number        VARCHAR(50),
    invoice_title     VARCHAR(255),
    credit_limit      NUMERIC(20,4),
    payment_terms     TEXT,
    receivable_account VARCHAR(100),
    owner_id          BIGINT,
    department_id     BIGINT,
    remark            TEXT        NOT NULL DEFAULT '',
    operator_id       BIGINT      NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at        TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_customers_customer_code ON customers (customer_code) WHERE deleted_at IS NULL;
CREATE INDEX idx_customers_status ON customers (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_customers_category ON customers (category) WHERE deleted_at IS NULL;
CREATE INDEX idx_customers_owner_id ON customers (owner_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_customers_department_id ON customers (department_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_customers_name_trgm ON customers USING gin (customer_name gin_trgm_ops);

-- ============================================================================
-- 2. Customer Contacts — 客户联系人
-- ============================================================================

CREATE TABLE customer_contacts (
    contact_id   BIGSERIAL   PRIMARY KEY,
    customer_id  BIGINT      NOT NULL,
    contact_name VARCHAR(100) NOT NULL,
    position     VARCHAR(100),
    phone        VARCHAR(50),
    email        VARCHAR(100),
    is_primary   BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_customer_contacts_customer_id ON customer_contacts (customer_id);

-- ============================================================================
-- 3. Customer Addresses — 客户地址
-- ============================================================================

CREATE TABLE customer_addresses (
    address_id    BIGSERIAL   PRIMARY KEY,
    customer_id   BIGINT      NOT NULL,
    address_type  VARCHAR(50) NOT NULL,
    province      VARCHAR(100) NOT NULL,
    city          VARCHAR(100) NOT NULL,
    district      VARCHAR(100),
    detail        TEXT        NOT NULL,
    contact_name  VARCHAR(100),
    contact_phone VARCHAR(50),
    is_default    BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_customer_addresses_customer_id ON customer_addresses (customer_id);

COMMIT;
