use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor, Result};

use super::model::*;

#[async_trait]
pub trait WorkCalendarService: Send + Sync {
    // ── 日历 CRUD ──
    async fn create_calendar(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateCalendarReq,
    ) -> Result<i64>;

    async fn get_calendar(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<WorkCalendar>;

    async fn list_calendars(&self, db: PgExecutor<'_>) -> Result<Vec<WorkCalendar>>;

    /// 替换日历的所有工作时间（对标 Odoo resource.calendar.attendance 批量设置）
    async fn set_lines(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        calendar_id: i64,
        lines: Vec<CalendarLineInput>,
    ) -> Result<()>;

    async fn list_lines(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        calendar_id: i64,
    ) -> Result<Vec<CalendarLine>>;

    // ── 例外日 ──
    async fn add_exception(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: AddExceptionReq,
    ) -> Result<i64>;

    async fn list_exceptions(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        calendar_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<CalendarException>>;

    // ── 排程核心 ──

    /// 在工作中心日历上找第一个可用时段
    /// 对标 Odoo workcenter._get_first_available_slot
    async fn find_available_slot(
        &self,
        db: PgExecutor<'_>,
        work_center_id: i64,
        from: DateTime<Utc>,
        duration_minutes: Decimal,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>>;

    /// 创建时段占用（排程时调用）
    /// 对标 Odoo resource.calendar.leaves.create
    async fn create_booking(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateBookingReq,
    ) -> Result<i64>;

    /// 取消工单的所有时段占用（反下达/取消时调用）
    async fn cancel_bookings_by_work_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<u64>;

    /// 查询工作中心在指定时段内的已有占用（负荷看板用）
    async fn list_bookings(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_center_id: i64,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<WorkCenterBooking>>;
    /// 批量查询多个工作中心的时段占用（甘特图用）
    async fn list_bookings_multi(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_center_ids: &[i64],
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<WorkCenterBooking>>;
}
