-- ============================================================================
-- 046: Work Center + Work Calendar infrastructure (对标 Odoo 排程基础设施)
-- 参考: Odoo resource.calendar / resource.calendar.attendance / resource.calendar.leaves
--       Odoo mrp.workcenter (costs_hour, time_efficiency, capacity, resource_calendar_id)
--       ERPNext Workstation / Workstation Working Hour
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. Work Calendars — 工作日历 (对标 Odoo resource.calendar)
-- ============================================================================

CREATE TABLE IF NOT EXISTS work_calendars (
    id          BIGSERIAL    PRIMARY KEY,
    name        VARCHAR(200) NOT NULL,
    description TEXT,
    operator_id BIGINT,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ,
    deleted_at  TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_work_calendars_name ON work_calendars (name);

-- ============================================================================
-- 2. Work Calendar Lines — 日历工作时间明细
--    对标 Odoo resource.calendar.attendance (dayofweek, hour_from, hour_to)
--    weekday: 0=周日 1=周一 ... 6=周六 (与 chrono::Weekday::num_days_from_sunday 一致)
-- ============================================================================

CREATE TABLE IF NOT EXISTS work_calendar_lines (
    id          BIGSERIAL PRIMARY KEY,
    calendar_id BIGINT    NOT NULL REFERENCES work_calendars(id),
    weekday     SMALLINT  NOT NULL,  -- 0-6 (Sun-Sat)
    from_time   TIME      NOT NULL,  -- 工作开始 如 08:00
    to_time     TIME      NOT NULL,  -- 工作结束 如 17:00
    sort_order  INT       NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_work_calendar_lines_calendar ON work_calendar_lines (calendar_id);
CREATE INDEX IF NOT EXISTS idx_work_calendar_lines_day ON work_calendar_lines (calendar_id, weekday);

-- ============================================================================
-- 3. Work Calendar Exceptions — 节假日 / 特殊工作日
--    is_workday=true: 特殊工作日(调休上班), 可指定工作时间
--    is_workday=false: 节假日休息
-- ============================================================================

CREATE TABLE IF NOT EXISTS work_calendar_exceptions (
    id              BIGSERIAL PRIMARY KEY,
    calendar_id     BIGINT NOT NULL REFERENCES work_calendars(id),
    exception_date  DATE   NOT NULL,
    is_workday      BOOLEAN NOT NULL DEFAULT false,
    from_time       TIME,
    to_time         TIME,
    remark          TEXT,
    UNIQUE(calendar_id, exception_date)
);

CREATE INDEX IF NOT EXISTS idx_work_calendar_exceptions_cal_date ON work_calendar_exceptions (calendar_id, exception_date);

-- ============================================================================
-- 4. Work Centers — 工作中心 (对标 Odoo mrp.workcenter)
--    此前 work_center_id 在 work_orders/routing_steps/work_order_routings 中悬空
--    现补齐实体表
-- ============================================================================

CREATE TABLE IF NOT EXISTS work_centers (
    id               BIGSERIAL     PRIMARY KEY,
    code             VARCHAR(50)   NOT NULL UNIQUE,
    name             VARCHAR(200)  NOT NULL,
    work_center_type SMALLINT      NOT NULL DEFAULT 1, -- 1=机器 2=人工 3=委外
    costs_hour       DECIMAL(18,6) NOT NULL DEFAULT 0,    -- 每小时成本 (对标 costs_hour)
    time_efficiency  DECIMAL(5,2)  NOT NULL DEFAULT 100.00, -- 效率系数% (对标 time_efficiency)
    setup_time       DECIMAL(18,6) NOT NULL DEFAULT 0,    -- 准备时间/分钟 (对标 time_start)
    cleanup_time     DECIMAL(18,6) NOT NULL DEFAULT 0,    -- 清理时间/分钟 (对标 time_stop)
    default_capacity DECIMAL(18,6) NOT NULL DEFAULT 1,    -- 默认并行产能 (对标 capacity)
    calendar_id      BIGINT REFERENCES work_calendars(id), -- 工作日历 (对标 resource_calendar_id)
    location         VARCHAR(200),
    is_active        BOOLEAN       NOT NULL DEFAULT true,
    operator_id      BIGINT,
    created_at       TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ,
    deleted_at       TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_work_centers_code ON work_centers (code);
CREATE INDEX IF NOT EXISTS idx_work_centers_active ON work_centers (is_active) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_work_centers_calendar ON work_centers (calendar_id);

-- ============================================================================
-- 5. Work Center Bookings — 工作中心时段占用 (对标 Odoo resource.calendar.leaves)
--    排程时创建 booking 占用时段，防止同一工作中心多工单时间重叠
--    _plan_workorder → find_available_slot → create_booking
-- ============================================================================

CREATE TABLE IF NOT EXISTS work_center_bookings (
    id               BIGSERIAL   PRIMARY KEY,
    work_center_id   BIGINT      NOT NULL REFERENCES work_centers(id),
    work_order_id    BIGINT      NOT NULL REFERENCES work_orders(id),
    plan_item_id     BIGINT,
    date_from        TIMESTAMPTZ NOT NULL,
    date_to          TIMESTAMPTZ NOT NULL,
    duration_minutes DECIMAL(18,6) NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 排程核心查询: 找可用时段时排除已有 booking
CREATE INDEX IF NOT EXISTS idx_work_center_bookings_wc_time ON work_center_bookings (work_center_id, date_from, date_to);
CREATE INDEX IF NOT EXISTS idx_work_center_bookings_wo ON work_center_bookings (work_order_id);

COMMIT;
