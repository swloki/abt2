use chrono::NaiveDate;
use sqlx::FromRow;

use super::model::*;

pub struct WorkReportRepo;

impl WorkReportRepo {
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<WorkReport>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, batch_id, routing_id,
                   report_date, shift, worker_id, completed_qty, defect_qty,
                   defect_reason, work_hours, remark, operator_id, created_at, updated_at
            FROM work_reports
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| WorkReport::from_row(&r)).transpose()
    }

    pub async fn list_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Vec<WorkReport>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, batch_id, routing_id,
                   report_date, shift, worker_id, completed_qty, defect_qty,
                   defect_reason, work_hours, remark, operator_id, created_at, updated_at
            FROM work_reports
            WHERE work_order_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(work_order_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| WorkReport::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    pub async fn list_by_batch(
        executor: &mut sqlx::postgres::PgConnection,
        batch_id: i64,
    ) -> Result<Vec<WorkReport>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, batch_id, routing_id,
                   report_date, shift, worker_id, completed_qty, defect_qty,
                   defect_reason, work_hours, remark, operator_id, created_at, updated_at
            FROM work_reports
            WHERE batch_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(batch_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| WorkReport::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    pub async fn list_by_worker_and_date_range(
        executor: &mut sqlx::postgres::PgConnection,
        worker_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<WorkReport>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, batch_id, routing_id,
                   report_date, shift, worker_id, completed_qty, defect_qty,
                   defect_reason, work_hours, remark, operator_id, created_at, updated_at
            FROM work_reports
            WHERE worker_id = $1 AND report_date >= $2 AND report_date <= $3
            ORDER BY report_date, created_at
            "#,
        )
        .bind(worker_id)
        .bind(from)
        .bind(to)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| WorkReport::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }
}
