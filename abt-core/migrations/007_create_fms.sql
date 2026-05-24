-- ============================================================================
-- ABT v2 Financial Management Module (FMS) — 5 Tables
-- Database: abt_v2
-- No FK constraints (application-enforced, per project convention)
-- All enums stored as SMALLINT (i16), application-enforced
-- Amounts: NUMERIC(20,4)
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Cash Journals — 出纳日记账主表
-- ============================================================================

CREATE TABLE cash_journals (
    id                  BIGSERIAL      PRIMARY KEY,
    doc_number          VARCHAR(32)    NOT NULL,
    journal_type        SMALLINT       NOT NULL, -- JournalType
    direction           SMALLINT       NOT NULL, -- CashDirection
    amount              NUMERIC(20,4)  NOT NULL,
    counterparty_type   SMALLINT       NOT NULL, -- CounterpartyType
    counterparty_id     BIGINT         NOT NULL,
    source_type         SMALLINT       NOT NULL, -- DocumentType
    source_id           BIGINT         NOT NULL,
    bank_account        VARCHAR(64)    NOT NULL DEFAULT '',
    transaction_date    DATE           NOT NULL,
    period              VARCHAR(7)     NOT NULL, -- "2026-05"
    status              SMALLINT       NOT NULL DEFAULT 1, -- JournalStatus::Draft
    remark              TEXT           NOT NULL DEFAULT '',
    operator_id         BIGINT         NOT NULL,
    version             INTEGER        NOT NULL DEFAULT 1,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_cj_doc_number ON cash_journals (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_cj_type_status ON cash_journals (journal_type, status) WHERE deleted_at IS NULL;
CREATE INDEX idx_cj_counterparty ON cash_journals (counterparty_type, counterparty_id);
CREATE INDEX idx_cj_period ON cash_journals (period) WHERE deleted_at IS NULL;
CREATE INDEX idx_cj_transaction_date ON cash_journals (transaction_date);

-- ============================================================================
-- 2. Cash Journal Lines — 日记账明细（双层记账）
-- ============================================================================

CREATE TABLE cash_journal_lines (
    id              BIGSERIAL      PRIMARY KEY,
    journal_id      BIGINT         NOT NULL,
    account_code    VARCHAR(32)    NOT NULL,
    debit_amount    NUMERIC(20,4)  NOT NULL DEFAULT 0,
    credit_amount   NUMERIC(20,4)  NOT NULL DEFAULT 0,
    cost_center     BIGINT,
    profit_center   BIGINT,
    remark          TEXT           NOT NULL DEFAULT ''
);

CREATE INDEX idx_cjl_journal ON cash_journal_lines (journal_id);

-- ============================================================================
-- 3. Write Offs — 核销记录
-- ============================================================================

CREATE TABLE write_offs (
    id                BIGSERIAL      PRIMARY KEY,
    write_off_type    SMALLINT       NOT NULL, -- WriteOffType
    cash_journal_id   BIGINT         NOT NULL,
    source_type       SMALLINT       NOT NULL, -- DocumentType
    source_id         BIGINT         NOT NULL,
    amount            NUMERIC(20,4)  NOT NULL CHECK (amount > 0),
    write_off_date    DATE           NOT NULL,
    idempotency_key   VARCHAR(128),
    operator_id       BIGINT         NOT NULL,
    created_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_wo_journal ON write_offs (cash_journal_id);
CREATE INDEX idx_wo_source ON write_offs (source_type, source_id);
CREATE UNIQUE INDEX uk_wo_idempotency ON write_offs (idempotency_key) WHERE idempotency_key IS NOT NULL;

-- ============================================================================
-- 4. Expense Reimbursements — 费用报销主表
-- ============================================================================

CREATE TABLE expense_reimbursements (
    id              BIGSERIAL      PRIMARY KEY,
    doc_number      VARCHAR(32)    NOT NULL,
    applicant_id    BIGINT         NOT NULL,
    department_id   BIGINT,
    expense_date    DATE           NOT NULL,
    total_amount    NUMERIC(20,4)  NOT NULL,
    status          SMALLINT       NOT NULL DEFAULT 1, -- ExpenseStatus::Draft
    remark          TEXT           NOT NULL DEFAULT '',
    operator_id     BIGINT         NOT NULL,
    version         INTEGER        NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_er_doc_number ON expense_reimbursements (doc_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_er_applicant ON expense_reimbursements (applicant_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_er_status ON expense_reimbursements (status) WHERE deleted_at IS NULL;

-- ============================================================================
-- 5. Expense Reimbursement Items — 费用报销明细
-- ============================================================================

CREATE TABLE expense_reimbursement_items (
    id                  BIGSERIAL      PRIMARY KEY,
    reimbursement_id    BIGINT         NOT NULL,
    expense_type        SMALLINT       NOT NULL, -- ExpenseType
    amount              NUMERIC(20,4)  NOT NULL,
    description         TEXT           NOT NULL DEFAULT '',
    receipt_no          VARCHAR(64),
    cost_center         BIGINT,
    profit_center       BIGINT
);

CREATE INDEX idx_eri_reimbursement ON expense_reimbursement_items (reimbursement_id);

COMMIT;
