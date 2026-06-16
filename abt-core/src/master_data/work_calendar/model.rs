use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;

// ============================================================================
// 工作日历
// ============================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkCalendar {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateCalendarReq {
    pub name: String,
    pub description: Option<String>,
}

// ============================================================================
// 日历工作时间明细 (对标 Odoo resource.calendar.attendance)
// ============================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CalendarLine {
    pub id: i64,
    pub calendar_id: i64,
    pub weekday: i16,      // 0=周日 1=周一 ... 6=周六
    pub from_time: NaiveTime,
    pub to_time: NaiveTime,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct CalendarLineInput {
    pub weekday: i16,
    pub from_time: NaiveTime,
    pub to_time: NaiveTime,
}

// ============================================================================
// 日历例外 (节假日/特殊工作日)
// ============================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CalendarException {
    pub id: i64,
    pub calendar_id: i64,
    pub exception_date: NaiveDate,
    pub is_workday: bool,
    pub from_time: Option<NaiveTime>,
    pub to_time: Option<NaiveTime>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddExceptionReq {
    pub calendar_id: i64,
    pub exception_date: NaiveDate,
    pub is_workday: bool,
    pub from_time: Option<NaiveTime>,
    pub to_time: Option<NaiveTime>,
    pub remark: Option<String>,
}

// ============================================================================
// 工作中心时段占用 (对标 Odoo resource.calendar.leaves)
// ============================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkCenterBooking {
    pub id: i64,
    pub work_center_id: i64,
    pub work_order_id: i64,
    pub plan_item_id: Option<i64>,
    pub date_from: DateTime<Utc>,
    pub date_to: DateTime<Utc>,
    pub duration_minutes: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateBookingReq {
    pub work_center_id: i64,
    pub work_order_id: i64,
    pub plan_item_id: Option<i64>,
    pub date_from: DateTime<Utc>,
    pub date_to: DateTime<Utc>,
    pub duration_minutes: Decimal,
}

/// 可用时段查找结果
pub type AvailableSlot = Option<(DateTime<Utc>, DateTime<Utc>)>;
