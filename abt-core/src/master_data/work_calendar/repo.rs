use chrono::{DateTime, NaiveDate, Utc};


use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;

// ============================================================================
// Calendar CRUD
// ============================================================================

pub struct CalendarRepo;

impl CalendarRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        req: &CreateCalendarReq,
        operator_id: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO work_calendars (name, description, operator_id)
               VALUES ($1, $2, $3) RETURNING id"#,
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn get_by_id(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<WorkCalendar>> {
        let row = sqlx::query_as::<_, WorkCalendar>(
            r#"SELECT id, name, description, operator_id, created_at, updated_at
               FROM work_calendars WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn list_all(&self, executor: PgExecutor<'_>) -> Result<Vec<WorkCalendar>> {
        let rows = sqlx::query_as::<_, WorkCalendar>(
            r#"SELECT id, name, description, operator_id, created_at, updated_at
               FROM work_calendars WHERE deleted_at IS NULL
               ORDER BY created_at DESC"#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}

// ============================================================================
// Calendar Lines
// ============================================================================

pub struct CalendarLineRepo;

impl CalendarLineRepo {
    pub async fn replace_all(
        &self,
        executor: PgExecutor<'_>,
        calendar_id: i64,
        lines: &[CalendarLineInput],
    ) -> Result<()> {
        sqlx::query("DELETE FROM work_calendar_lines WHERE calendar_id = $1")
            .bind(calendar_id)
            .execute(&mut *executor)
            .await?;

        for (i, line) in lines.iter().enumerate() {
            sqlx::query(
                r#"INSERT INTO work_calendar_lines (calendar_id, weekday, from_time, to_time, sort_order)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(calendar_id)
            .bind(line.weekday)
            .bind(line.from_time)
            .bind(line.to_time)
            .bind(i as i32)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn list_by_calendar(
        &self,
        executor: PgExecutor<'_>,
        calendar_id: i64,
    ) -> Result<Vec<CalendarLine>> {
        let rows = sqlx::query_as::<_, CalendarLine>(
            r#"SELECT id, calendar_id, weekday, from_time, to_time, sort_order
               FROM work_calendar_lines WHERE calendar_id = $1 ORDER BY weekday, sort_order"#,
        )
        .bind(calendar_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}

// ============================================================================
// Calendar Exceptions
// ============================================================================

pub struct ExceptionRepo;

impl ExceptionRepo {
    pub async fn add(&self, executor: PgExecutor<'_>, req: &AddExceptionReq) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO work_calendar_exceptions
                 (calendar_id, exception_date, is_workday, from_time, to_time, remark)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (calendar_id, exception_date) DO UPDATE
                 SET is_workday = EXCLUDED.is_workday,
                     from_time = EXCLUDED.from_time,
                     to_time = EXCLUDED.to_time,
                     remark = EXCLUDED.remark
               RETURNING id"#,
        )
        .bind(req.calendar_id)
        .bind(req.exception_date)
        .bind(req.is_workday)
        .bind(req.from_time)
        .bind(req.to_time)
        .bind(&req.remark)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn list_range(
        &self,
        executor: PgExecutor<'_>,
        calendar_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<CalendarException>> {
        let rows = sqlx::query_as::<_, CalendarException>(
            r#"SELECT id, calendar_id, exception_date, is_workday, from_time, to_time, remark
               FROM work_calendar_exceptions
               WHERE calendar_id = $1 AND exception_date BETWEEN $2 AND $3
               ORDER BY exception_date"#,
        )
        .bind(calendar_id)
        .bind(from)
        .bind(to)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}

// ============================================================================
// Work Center Bookings (对标 Odoo resource.calendar.leaves)
// ============================================================================

pub struct BookingRepo;

impl BookingRepo {
    pub async fn create(&self, executor: PgExecutor<'_>, req: &CreateBookingReq) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO work_center_bookings
                 (work_center_id, work_order_id, plan_item_id, date_from, date_to, duration_minutes)
               VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
        )
        .bind(req.work_center_id)
        .bind(req.work_order_id)
        .bind(req.plan_item_id)
        .bind(req.date_from)
        .bind(req.date_to)
        .bind(req.duration_minutes)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn list_range(
        &self,
        executor: PgExecutor<'_>,
        work_center_id: i64,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<WorkCenterBooking>> {
        let rows = sqlx::query_as::<_, WorkCenterBooking>(
            r#"SELECT id, work_center_id, work_order_id, plan_item_id,
                      date_from, date_to, duration_minutes, created_at
               FROM work_center_bookings
               WHERE work_center_id = $1 AND date_from < $2 AND date_to > $3
               ORDER BY date_from"#,
        )
        .bind(work_center_id)
        .bind(to)
        .bind(from)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 批量查询多个工作中心的时段占用（甘特图用，避免 N+1）
    pub async fn list_range_multi(
        &self,
        executor: PgExecutor<'_>,
        work_center_ids: &[i64],
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<WorkCenterBooking>> {
        let rows = sqlx::query_as::<_, WorkCenterBooking>(
            r#"SELECT id, work_center_id, work_order_id, plan_item_id,
                      date_from, date_to, duration_minutes, created_at
               FROM work_center_bookings
               WHERE work_center_id = ANY($1) AND date_from < $2 AND date_to > $3
               ORDER BY work_center_id, date_from"#,
        )
        .bind(work_center_ids)
        .bind(to)
        .bind(from)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    pub async fn cancel_by_work_order(
        &self,
        executor: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM work_center_bookings WHERE work_order_id = $1",
        )
        .bind(work_order_id)
        .execute(&mut *executor)
        .await?;
        Ok(result.rows_affected())
    }
}
