-- ============================================================================
-- Notifications & Scheduled Tasks — 通知与定时任务
-- Database: abt_v2
-- All enums stored as SMALLINT (i16), application-enforced
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Notifications — 通知
-- ============================================================================

CREATE TABLE notifications (
    notification_id   BIGSERIAL   PRIMARY KEY,
    user_id           BIGINT      NOT NULL,
    notification_type SMALLINT    NOT NULL,           -- 1=System, 2=Business, 3=Alert
    title             VARCHAR(255) NOT NULL,
    content           TEXT,
    related_type      VARCHAR(100),
    related_id        BIGINT,
    is_read           BOOLEAN     NOT NULL DEFAULT FALSE,
    read_at           TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_notifications_user_id ON notifications (user_id);
CREATE INDEX idx_notifications_user_type ON notifications (user_id, notification_type);
CREATE INDEX idx_notifications_user_read ON notifications (user_id, is_read);

-- ============================================================================
-- 2. Scheduled Task Defs — 定时任务定义
-- ============================================================================

CREATE TABLE scheduled_task_defs (
    task_id         BIGSERIAL   PRIMARY KEY,
    name            VARCHAR(255) NOT NULL,
    interval_secs   BIGINT      NOT NULL,
    timeout_secs    BIGINT      NOT NULL,
    is_enabled      BOOLEAN     NOT NULL DEFAULT TRUE,
    last_run_at     TIMESTAMPTZ,
    last_elapsed_ms BIGINT,
    last_result     TEXT,
    last_error      TEXT,
    total_runs      BIGINT      NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX idx_scheduled_task_defs_name ON scheduled_task_defs (name);

-- ============================================================================
-- 3. Task Run Logs — 任务执行记录
-- ============================================================================

CREATE TABLE task_run_logs (
    run_id      BIGSERIAL   PRIMARY KEY,
    task_id     BIGINT      NOT NULL,
    status      SMALLINT    NOT NULL,           -- 1=Pending, 2=Running, 3=Completed, 4=Failed
    started_at  TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    elapsed_ms  BIGINT,
    result      TEXT,
    error       TEXT
);

CREATE INDEX idx_task_run_logs_task_id ON task_run_logs (task_id);
CREATE INDEX idx_task_run_logs_started_at ON task_run_logs (started_at DESC);

COMMIT;
