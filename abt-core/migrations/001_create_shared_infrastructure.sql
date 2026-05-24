-- ============================================================================
-- ABT v2 Shared Infrastructure — Initial Schema
-- Database: abt_v2 (independent, no FK constraints)
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- Extensions
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ============================================================================
-- 1. Document Sequence — 单据编号序列
-- ============================================================================

CREATE TABLE document_sequences (
    id          BIGSERIAL   PRIMARY KEY,
    prefix      VARCHAR(20) NOT NULL,         -- "SO", "PO", "WO"
    current_value INTEGER    NOT NULL DEFAULT 0,
    seq_date    DATE        NOT NULL,          -- 按日期分段
    padding_len INTEGER     NOT NULL DEFAULT 4,-- 补零位数
    strategy    SMALLINT    NOT NULL DEFAULT 1,-- 1=Sequential, 2=Timestamp
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (prefix, seq_date)
);

-- ============================================================================
-- 2. Document Link — 单据关联
-- ============================================================================

CREATE TABLE document_links (
    id          BIGSERIAL   PRIMARY KEY,
    source_type SMALLINT    NOT NULL,          -- DocumentType i16
    source_id   BIGINT      NOT NULL,
    target_type SMALLINT    NOT NULL,          -- DocumentType i16
    target_id   BIGINT      NOT NULL,
    link_type   SMALLINT    NOT NULL,          -- LinkType i16
    path        VARCHAR(255) NOT NULL DEFAULT '', -- 物化路径 "SO.42.SR.15"
    depth       INTEGER     NOT NULL DEFAULT 1,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by  BIGINT
);

CREATE INDEX idx_doc_links_source ON document_links (source_type, source_id);
CREATE INDEX idx_doc_links_target ON document_links (target_type, target_id);
CREATE INDEX idx_doc_links_path ON document_links USING gin (path gin_trgm_ops);

-- ============================================================================
-- 3. Inventory Reservation — 库存预留
-- ============================================================================

CREATE TABLE inventory_reservations (
    id               BIGSERIAL      PRIMARY KEY,
    product_id       BIGINT         NOT NULL,
    warehouse_id     BIGINT         NOT NULL,
    reserved_qty     NUMERIC(18,6)  NOT NULL,
    reservation_type SMALLINT       NOT NULL,   -- 1=Hard, 2=Soft, 3=SafetyStock
    source_type      SMALLINT       NOT NULL,   -- DocumentType i16
    source_id        BIGINT         NOT NULL,
    source_line_id   BIGINT,                     -- 单据行级精确释放
    status           SMALLINT       NOT NULL DEFAULT 1, -- 1=Active, 2=Fulfilled, 3=Cancelled, 4=Expired
    priority         INTEGER        NOT NULL DEFAULT 5,
    expires_at       TIMESTAMPTZ,
    created_at       TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_inv_res_product ON inventory_reservations (product_id, warehouse_id, status);
CREATE INDEX idx_inv_res_source  ON inventory_reservations (source_type, source_id);

-- ============================================================================
-- 4. Cost Entry — 成本分录
-- ============================================================================

CREATE TABLE cost_entries (
    id            BIGSERIAL      PRIMARY KEY,
    entity_type   SMALLINT       NOT NULL,     -- CostEntityType i16
    entity_id     BIGINT         NOT NULL,
    cost_type     SMALLINT       NOT NULL,     -- CostType i16
    debit_amount  NUMERIC(20,4)  NOT NULL DEFAULT 0,
    credit_amount NUMERIC(20,4)  NOT NULL DEFAULT 0,
    cost_center   BIGINT,                      -- 部门/产线
    profit_center BIGINT,
    period        VARCHAR(7)     NOT NULL,     -- "2026-05"
    source_type   SMALLINT       NOT NULL,     -- DocumentType i16
    source_id     BIGINT         NOT NULL,
    created_at    TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cost_entries_entity ON cost_entries (entity_type, entity_id);
CREATE INDEX idx_cost_entries_period  ON cost_entries (period);

-- ============================================================================
-- 5. Domain Events — 领域事件 Outbox
-- ============================================================================

CREATE TABLE domain_events (
    id              BIGSERIAL    PRIMARY KEY,
    event_type      SMALLINT     NOT NULL,       -- DomainEventType i16
    event_version   INTEGER      NOT NULL DEFAULT 1,
    aggregate_type  VARCHAR(50)  NOT NULL,       -- "SalesOrder"
    aggregate_id    BIGINT       NOT NULL,
    payload         JSONB        NOT NULL DEFAULT '{}',
    operator_id     BIGINT       NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL,
    trace_id        VARCHAR(255),
    request_id      VARCHAR(255),
    status          SMALLINT     NOT NULL DEFAULT 1, -- 1=Pending, 2=Processing, 3=Processed, 4=Failed, 5=DeadLetter
    retry_count     INTEGER      NOT NULL DEFAULT 0,
    failure_reason  TEXT,
    processed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    UNIQUE (idempotency_key)
);

CREATE INDEX idx_domain_events_status     ON domain_events (status, created_at);
CREATE INDEX idx_domain_events_aggregate  ON domain_events (aggregate_type, aggregate_id);

-- ============================================================================
-- 6. State Machine — 状态机
-- ============================================================================

CREATE TABLE state_definitions (
    id          BIGSERIAL   PRIMARY KEY,
    entity_type VARCHAR(50) NOT NULL,
    state_name  VARCHAR(50) NOT NULL,
    label       VARCHAR(100) NOT NULL,
    is_initial  BOOLEAN     NOT NULL DEFAULT FALSE,
    is_final    BOOLEAN     NOT NULL DEFAULT FALSE,

    UNIQUE (entity_type, state_name)
);

CREATE TABLE state_transition_defs (
    id              BIGSERIAL   PRIMARY KEY,
    entity_type     VARCHAR(50) NOT NULL,
    from_state      VARCHAR(50) NOT NULL,
    to_state        VARCHAR(50) NOT NULL,
    trigger_event   SMALLINT,                   -- DomainEventType i16
    guard_condition JSONB,
    side_effects    JSONB       NOT NULL DEFAULT '[]',
    sort_order      INTEGER     NOT NULL DEFAULT 0,

    UNIQUE (entity_type, from_state, to_state)
);

CREATE INDEX idx_state_trans ON state_transition_defs (entity_type, from_state);

CREATE TABLE entity_state_logs (
    id            BIGSERIAL   PRIMARY KEY,
    entity_type   VARCHAR(50) NOT NULL,
    entity_id     BIGINT      NOT NULL,
    from_state    VARCHAR(50),
    to_state      VARCHAR(50) NOT NULL,
    transition_id BIGINT      NOT NULL,
    operator_id   BIGINT      NOT NULL,
    remark        VARCHAR(500),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_state_logs_entity ON entity_state_logs (entity_type, entity_id, created_at DESC);

-- ============================================================================
-- 7. Audit Log — 审计日志（按月分区）
-- ============================================================================

CREATE TABLE audit_logs (
    id          BIGSERIAL,
    entity_type VARCHAR(50) NOT NULL,
    entity_id   BIGINT      NOT NULL,
    action      SMALLINT    NOT NULL,          -- AuditAction i16
    changes     JSONB,
    operator_id BIGINT      NOT NULL,
    context     JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

-- 初始分区：2026 年各月
CREATE TABLE audit_logs_2026_01 PARTITION OF audit_logs FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');
CREATE TABLE audit_logs_2026_02 PARTITION OF audit_logs FOR VALUES FROM ('2026-02-01') TO ('2026-03-01');
CREATE TABLE audit_logs_2026_03 PARTITION OF audit_logs FOR VALUES FROM ('2026-03-01') TO ('2026-04-01');
CREATE TABLE audit_logs_2026_04 PARTITION OF audit_logs FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');
CREATE TABLE audit_logs_2026_05 PARTITION OF audit_logs FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
CREATE TABLE audit_logs_2026_06 PARTITION OF audit_logs FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');
CREATE TABLE audit_logs_2026_07 PARTITION OF audit_logs FOR VALUES FROM ('2026-07-01') TO ('2026-08-01');
CREATE TABLE audit_logs_2026_08 PARTITION OF audit_logs FOR VALUES FROM ('2026-08-01') TO ('2026-09-01');
CREATE TABLE audit_logs_2026_09 PARTITION OF audit_logs FOR VALUES FROM ('2026-09-01') TO ('2026-10-01');
CREATE TABLE audit_logs_2026_10 PARTITION OF audit_logs FOR VALUES FROM ('2026-10-01') TO ('2026-11-01');
CREATE TABLE audit_logs_2026_11 PARTITION OF audit_logs FOR VALUES FROM ('2026-11-01') TO ('2026-12-01');
CREATE TABLE audit_logs_2026_12 PARTITION OF audit_logs FOR VALUES FROM ('2026-12-01') TO ('2027-01-01');

CREATE INDEX idx_audit_logs_entity ON audit_logs (entity_type, entity_id);
CREATE INDEX idx_audit_logs_operator ON audit_logs (operator_id);
CREATE INDEX idx_audit_logs_created_at ON audit_logs (created_at);

-- ============================================================================
-- 8. Idempotency Records — 幂等记录
-- ============================================================================

CREATE TABLE idempotency_records (
    id              BIGSERIAL    PRIMARY KEY,
    idempotency_key VARCHAR(255) NOT NULL,
    event_id        BIGINT       NOT NULL,
    handler_name    VARCHAR(100) NOT NULL,
    status          VARCHAR(20)  NOT NULL DEFAULT 'Processing', -- Processing / Processed
    result          JSONB,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ,

    UNIQUE (idempotency_key)
);

CREATE INDEX idx_idempotency_event_handler ON idempotency_records (event_id, handler_name);

-- ============================================================================
-- 9. Identity & Access — 用户/角色/部门
-- ============================================================================

CREATE TABLE users (
    user_id        BIGSERIAL    PRIMARY KEY,
    username       VARCHAR(50)  NOT NULL,
    password_hash  VARCHAR(255) NOT NULL,
    display_name   VARCHAR(100),
    is_active      BOOLEAN      NOT NULL DEFAULT TRUE,
    is_super_admin BOOLEAN      NOT NULL DEFAULT FALSE,
    created_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ,

    UNIQUE (username)
);

CREATE TABLE roles (
    role_id        BIGSERIAL    PRIMARY KEY,
    role_name      VARCHAR(100) NOT NULL,
    role_code      VARCHAR(50)  NOT NULL,
    is_system_role BOOLEAN      NOT NULL DEFAULT FALSE,
    parent_role_id BIGINT,
    description    VARCHAR(255),
    created_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ,

    UNIQUE (role_code)
);

CREATE TABLE departments (
    department_id   BIGSERIAL    PRIMARY KEY,
    department_name VARCHAR(100) NOT NULL,
    department_code VARCHAR(50)  NOT NULL,
    description     VARCHAR(255),
    is_active       BOOLEAN      NOT NULL DEFAULT TRUE,
    is_default      BOOLEAN      NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ,

    UNIQUE (department_code)
);

CREATE TABLE user_roles (
    user_id BIGINT   NOT NULL,
    role_id BIGINT   NOT NULL,

    PRIMARY KEY (user_id, role_id)
);

CREATE TABLE user_departments (
    user_id       BIGINT NOT NULL,
    department_id BIGINT NOT NULL,

    PRIMARY KEY (user_id, department_id)
);

CREATE TABLE role_permissions (
    role_id       BIGINT      NOT NULL,
    resource_code VARCHAR(50) NOT NULL,
    action        VARCHAR(20) NOT NULL,    -- "create" / "read" / "update" / "delete"

    PRIMARY KEY (role_id, resource_code, action)
);

-- ============================================================================
-- 10. Seed Data — 超级管理员 + 基础角色
-- ============================================================================

-- Default password: "admin123" — argon2 hash will be set on first run
-- Insert placeholder; real password set via application
INSERT INTO users (username, password_hash, display_name, is_super_admin)
VALUES ('admin', '$argon2id$v=19$m=19456,t=2,p=1$placeholder', 'Super Admin', TRUE);

-- System roles
INSERT INTO roles (role_name, role_code, is_system_role, description) VALUES
    ('Super Admin',  'super_admin',  TRUE, 'System super administrator'),
    ('Admin',        'admin',        TRUE, 'System administrator'),
    ('Viewer',       'viewer',       TRUE, 'Read-only access');

-- Default department
INSERT INTO departments (department_name, department_code, is_default) VALUES
    ('Default', 'DEFAULT', TRUE);

COMMIT;
