use chrono::NaiveDate;
use sqlx::FromRow;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::*;

pub struct WorkReportRepo;

impl WorkReportRepo {
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<WorkReport>> {
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

        row.map(|r| WorkReport::from_row(&r).map_err(Into::into)).transpose()

    }

    pub async fn list_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Vec<WorkReport>> {
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
    ) -> Result<Vec<WorkReport>> {
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
    ) -> Result<Vec<WorkReport>> {
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
        Ok(rows.iter()
            .filter_map(|r| WorkReport::from_row(r).ok())
            .collect::<Vec<_>>())
    }

    pub async fn list_by_date_range(
        executor: &mut sqlx::postgres::PgConnection,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<WorkReport>> {
        let rows = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, batch_id, routing_id,
                   report_date, shift, worker_id, completed_qty, defect_qty,
                   defect_reason, work_hours, remark, operator_id, created_at, updated_at
            FROM work_reports
            WHERE report_date >= $1 AND report_date <= $2
            ORDER BY worker_id, report_date, created_at
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(&mut *executor)
        .await?;
        Ok(rows.iter()
            .filter_map(|r| WorkReport::from_row(r).ok())
            .collect::<Vec<_>>())
    }
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &ReportListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ReportListItem>> {
        let offset = page.saturating_sub(1) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 1u32;

        let keyword_param = if let Some(k) = &filter.keyword {
            if !k.is_empty() {
                where_clauses.push(format!("wr.doc_number ILIKE ${param_idx}"));
                param_idx += 1;
                Some(format!("%{k}%"))
            } else {
                None
            }
        } else {
            None
        };

        if let Some(_wo_id) = filter.work_order_id {
            where_clauses.push(format!("wr.work_order_id = ${param_idx}"));
            param_idx += 1;
        }

        if let Some(_s) = filter.shift {
            where_clauses.push(format!("wr.shift = ${param_idx}"));
            param_idx += 1;
        }

        if let Some(_d) = filter.date_from {
            where_clauses.push(format!("wr.report_date >= ${param_idx}"));
            param_idx += 1;
        }

        if let Some(_d) = filter.date_to {
            where_clauses.push(format!("wr.report_date <= ${param_idx}"));
            param_idx += 1;
        }

        let where_sql = where_clauses.join(" AND ");

        // count
        let count_sql = format!("SELECT COUNT(*)::bigint FROM work_reports wr WHERE {where_sql}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(k) = &keyword_param { count_q = count_q.bind(k); }
        if let Some(v) = filter.work_order_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.shift { count_q = count_q.bind(v); }
        if let Some(v) = filter.date_from { count_q = count_q.bind(v); }
        if let Some(v) = filter.date_to { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // data
        let limit_idx = param_idx;
        let offset_idx = param_idx + 1;
        let data_sql = format!(
            r#"
            SELECT wr.id, wr.doc_number,
                   wr.work_order_id, wr.batch_id,
                   wo.product_id,
                   p.pdt_name AS product_name,
                   wor.process_name,
                   wor.step_no AS step_order,
                   wr.report_date, wr.shift,
                   wr.worker_id,
                   u.display_name AS worker_name,
                   wr.completed_qty, wr.defect_qty,
                   wr.work_hours, wr.remark,
                   wr.operator_id, wr.created_at
            FROM work_reports wr
            JOIN work_orders wo ON wo.id = wr.work_order_id
            JOIN products p ON p.product_id = wo.product_id
            JOIN work_order_routings wor ON wor.id = wr.routing_id
            LEFT JOIN users u ON u.user_id = wr.worker_id
            WHERE {where_sql}
            ORDER BY wr.report_date DESC, wr.id DESC
            LIMIT ${limit_idx} OFFSET ${offset_idx}
            "#
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, ReportListItem>(sqlx::AssertSqlSafe(data_sql));
        if let Some(k) = &keyword_param { data_q = data_q.bind(k); }
        if let Some(v) = filter.work_order_id { data_q = data_q.bind(v); }
        if let Some(v) = filter.shift { data_q = data_q.bind(v); }
        if let Some(v) = filter.date_from { data_q = data_q.bind(v); }
        if let Some(v) = filter.date_to { data_q = data_q.bind(v); }
        data_q = data_q.bind(page_size as i64).bind(offset as i64);
        let items = data_q.fetch_all(&mut *executor).await?;

        Ok(PaginatedResult::new(items, total, page, page_size))
    }
}
