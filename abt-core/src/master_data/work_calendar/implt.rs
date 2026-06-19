use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::model::*;
use super::repo::{BookingRepo, CalendarLineRepo, CalendarRepo, ExceptionRepo};
use super::service::WorkCalendarService;
use crate::master_data::work_center::repo::WorkCenterRepo;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::{PgExecutor, Result};

pub struct WorkCalendarServiceImpl {
    _pool: PgPool,
}

impl WorkCalendarServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { _pool: pool }
    }
}

/// 获取下一天 00:00 UTC
fn next_day_midnight(date: NaiveDate) -> DateTime<Utc> {
    (date + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
}

#[async_trait]
impl WorkCalendarService for WorkCalendarServiceImpl {
    async fn create_calendar(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateCalendarReq,
    ) -> Result<i64> {
        CalendarRepo.create(db, &req, ctx.operator_id).await
    }

    async fn get_calendar(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<WorkCalendar> {
        CalendarRepo
            .get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkCalendar"))
    }

    async fn list_calendars(&self, db: PgExecutor<'_>) -> Result<Vec<WorkCalendar>> {
        CalendarRepo.list_all(db).await
    }

    async fn set_lines(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        calendar_id: i64,
        lines: Vec<CalendarLineInput>,
    ) -> Result<()> {
        // 校验日历存在
        let cal = CalendarRepo
            .get_by_id(db, calendar_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkCalendar"))?;
        let _ = cal; // 仅校验存在

        CalendarLineRepo.replace_all(db, calendar_id, &lines).await
    }

    async fn list_lines(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        calendar_id: i64,
    ) -> Result<Vec<CalendarLine>> {
        CalendarLineRepo.list_by_calendar(db, calendar_id).await
    }

    async fn add_exception(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: AddExceptionReq,
    ) -> Result<i64> {
        ExceptionRepo.add(db, &req).await
    }

    async fn list_exceptions(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        calendar_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<CalendarException>> {
        ExceptionRepo
            .list_range(db, calendar_id, from, to)
            .await
    }

    /// 核心算法：在工作中心日历上找第一个可用时段
    /// 对标 Odoo workcenter._get_first_available_slot
    async fn find_available_slot(
        &self,
        db: PgExecutor<'_>,
        work_center_id: i64,
        from: DateTime<Utc>,
        duration_minutes: Decimal,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>> {
        // 1. 查工作中心 → calendar_id
        let wc = WorkCenterRepo
            .get_by_id(db, work_center_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkCenter"))?;

        let calendar_id = wc.calendar_id.ok_or_else(|| {
            DomainError::BusinessRule(format!("工作中心 {} 未关联工作日历", wc.name))
        })?;

        let duration_min = duration_minutes.to_i64().unwrap_or(i64::MAX);
        let duration = Duration::minutes(duration_min);
        let scan_end = from + Duration::days(90);

        // 2. 预加载所有数据（避免循环内查询）
        let lines = CalendarLineRepo
            .list_by_calendar(db, calendar_id)
            .await?;
        let exceptions = ExceptionRepo
            .list_range(db, calendar_id, from.date_naive(), scan_end.date_naive())
            .await?;
        let bookings = BookingRepo
            .list_range(db, work_center_id, from, scan_end)
            .await?;

        // 3. 逐日扫描
        let mut current = from;

        while current < scan_end {
            let date = current.date_naive();
            let weekday = date.weekday().num_days_from_sunday() as i16;

            // 3a. 节假日跳过
            let is_holiday = exceptions
                .iter()
                .any(|e| e.exception_date == date && !e.is_workday);
            if is_holiday {
                current = next_day_midnight(date);
                continue;
            }

            // 3b. 查当日工作时段
            let day_lines: Vec<&CalendarLine> =
                lines.iter().filter(|l| l.weekday == weekday).collect();

            if day_lines.is_empty() {
                current = next_day_midnight(date);
                continue;
            }

            // 3c. 对每个工作时段找可用间隙
            for line in &day_lines {
                let period_start = date.and_time(line.from_time).and_utc();
                let period_end = date.and_time(line.to_time).and_utc();

                // 从 max(current, period_start) 开始
                let slot_from = if current > period_start {
                    current
                } else {
                    period_start
                };
                if slot_from >= period_end {
                    continue;
                }

                // 收集重叠 bookings，裁剪到当前时段
                let mut overlapping: Vec<(DateTime<Utc>, DateTime<Utc>)> = bookings
                    .iter()
                    .filter(|b| b.date_from < period_end && b.date_to > slot_from)
                    .map(|b| {
                        let s = if b.date_from > slot_from {
                            b.date_from
                        } else {
                            slot_from
                        };
                        let e = if b.date_to < period_end {
                            b.date_to
                        } else {
                            period_end
                        };
                        (s, e)
                    })
                    .collect();
                overlapping.sort();

                // 遍历 bookings 之间的间隙
                let mut gap_start = slot_from;
                for &(bk_start, bk_end) in &overlapping {
                    let gap = bk_start - gap_start;
                    if gap >= duration {
                        return Ok(Some((gap_start, gap_start + duration)));
                    }
                    if bk_end > gap_start {
                        gap_start = bk_end;
                    }
                }

                // 最后一个 booking 之后的间隙
                let final_gap = period_end - gap_start;
                if final_gap >= duration {
                    return Ok(Some((gap_start, gap_start + duration)));
                }
            }

            current = next_day_midnight(date);
        }

        Ok(None)
    }

    async fn create_booking(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateBookingReq,
    ) -> Result<i64> {
        BookingRepo.create(db, &req).await
    }

    async fn cancel_bookings_by_work_order(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<u64> {
        BookingRepo.cancel_by_work_order(db, work_order_id).await
    }

    async fn list_bookings(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_center_id: i64,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<WorkCenterBooking>> {
        BookingRepo.list_range(db, work_center_id, from, to).await
    }

    async fn list_bookings_multi(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_center_ids: &[i64],
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<WorkCenterBooking>> {
        BookingRepo
            .list_range_multi(db, work_center_ids, from, to)
            .await
    }
}
