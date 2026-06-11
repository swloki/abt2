use super::model::*;
use crate::shared::types::Result;
use sqlx::FromRow;

pub struct ProductionExceptionRepo;

impl ProductionExceptionRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        params: &CreateExceptionParams<'_>,
    ) -> Result<ProductionException> {
        let row = sqlx::query(
            "INSERT INTO production_exceptions \
             (doc_number, exception_type, status, severity, reason_category, \
              work_order_id, batch_id, product_id, current_step, impact_qty, \
              description, disposition, found_at, finder_id, owner_id, operator_id) \
             VALUES ($1, $2, 1, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) \
             RETURNING id, doc_number, exception_type, status, severity, reason_category, \
              work_order_id, batch_id, product_id, current_step, impact_qty, \
              description, disposition, found_at, finder_id, owner_id, operator_id, \
              created_at, updated_at"
        )
        .bind(params.doc_number)
        .bind(params.exception_type)
        .bind(params.severity)
        .bind(params.reason_category)
        .bind(params.work_order_id)
        .bind(params.batch_id)
        .bind(params.product_id)
        .bind(params.current_step)
        .bind(params.impact_qty)
        .bind(params.description)
        .bind(params.disposition)
        .bind(params.found_at)
        .bind(params.finder_id)
        .bind(params.owner_id)
        .bind(params.operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(ProductionException::from_row(&row)?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<ProductionException>> {
        let row = sqlx::query(
            "SELECT id, doc_number, exception_type, status, severity, reason_category, \
             work_order_id, batch_id, product_id, current_step, impact_qty, \
             description, disposition, found_at, finder_id, owner_id, operator_id, \
             created_at, updated_at \
             FROM production_exceptions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| ProductionException::from_row(&r).map_err(Into::into)).transpose()
    }

    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &ExceptionListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<ExceptionListItem>, i64)> {
        let offset = (page.saturating_sub(1)) * page_size;
        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx: i32 = 0;

        if filter.exception_type.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pe.exception_type = ${param_idx}"));
        }
        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pe.status = ${param_idx}"));
        }
        if filter.reason_category.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pe.reason_category = ${param_idx}"));
        }
        if let Some(kw) = &filter.keyword
            && !kw.is_empty() {
                param_idx += 1;
                where_clauses.push(format!(
                    "(pe.doc_number ILIKE ${param_idx} OR pe.description ILIKE ${param_idx})"
                ));
            }
        if filter.date_from.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pe.found_at::date >= ${param_idx}"));
        }
        if filter.date_to.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pe.found_at::date <= ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");

        // Count
        let count_sql = format!(
            "SELECT COUNT(*)::bigint FROM production_exceptions pe WHERE {where_sql}"
        );
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(t) = filter.exception_type { count_q = count_q.bind(t.as_i16()); }
        if let Some(s) = filter.status { count_q = count_q.bind(s.as_i16()); }
        if let Some(r) = filter.reason_category { count_q = count_q.bind(r.as_i16()); }
        if let Some(kw) = &filter.keyword
            && !kw.is_empty() { count_q = count_q.bind(format!("%{kw}%")); }
        if let Some(d) = filter.date_from { count_q = count_q.bind(d); }
        if let Some(d) = filter.date_to { count_q = count_q.bind(d); }
        let total = count_q.fetch_one(&mut *executor).await?;

        // Data
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;
        let data_sql = format!(
            "SELECT pe.id, pe.doc_number, pe.exception_type, pe.status, pe.reason_category, \
             pe.work_order_id, pe.batch_id, \
             wo.doc_number AS wo_doc_number, \
             pb.batch_no, \
             p.pdt_name AS product_name, \
             pe.description, pe.impact_qty, pe.found_at, pe.created_at \
             FROM production_exceptions pe \
             LEFT JOIN work_orders wo ON wo.id = pe.work_order_id \
             LEFT JOIN production_batches pb ON pb.id = pe.batch_id \
             LEFT JOIN products p ON p.product_id = pe.product_id \
             WHERE {where_sql} \
             ORDER BY pe.found_at DESC \
             LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, ExceptionListItem>(sqlx::AssertSqlSafe(data_sql));
        if let Some(t) = filter.exception_type { data_q = data_q.bind(t.as_i16()); }
        if let Some(s) = filter.status { data_q = data_q.bind(s.as_i16()); }
        if let Some(r) = filter.reason_category { data_q = data_q.bind(r.as_i16()); }
        if let Some(kw) = &filter.keyword
            && !kw.is_empty() { data_q = data_q.bind(format!("%{kw}%")); }
        if let Some(d) = filter.date_from { data_q = data_q.bind(d); }
        if let Some(d) = filter.date_to { data_q = data_q.bind(d); }
        data_q = data_q.bind(page_size as i64).bind(offset as i64);
        let items = data_q.fetch_all(&mut *executor).await?;

        Ok((items, total))
    }

    pub async fn get_stats(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<ExceptionStats> {
        let total_month: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM production_exceptions \
             WHERE EXTRACT(MONTH FROM found_at) = EXTRACT(MONTH FROM CURRENT_DATE) \
             AND EXTRACT(YEAR FROM found_at) = EXTRACT(YEAR FROM CURRENT_DATE)"
        ).fetch_one(&mut *executor).await?;

        let suspended: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM production_exceptions WHERE exception_type = 1 AND status != 3"
        ).fetch_one(&mut *executor).await?;

        let scrapped: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM production_exceptions WHERE exception_type = 2 AND status != 3"
        ).fetch_one(&mut *executor).await?;

        let insp_failed: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM production_exceptions WHERE exception_type = 4 AND status != 3"
        ).fetch_one(&mut *executor).await?;

        Ok(ExceptionStats {
            total_month: total_month.0,
            batch_suspended: suspended.0,
            batch_scrapped: scrapped.0,
            inspection_failed: insp_failed.0,
        })
    }

    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: super::super::enums::ExceptionStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE production_exceptions SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(status)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    // ── Events ──

    pub async fn insert_event(
        executor: &mut sqlx::postgres::PgConnection,
        exception_id: i64,
        event_type: &str,
        description: Option<&str>,
        operator_id: i64,
    ) -> Result<ExceptionEvent> {
        let row = sqlx::query(
            "INSERT INTO production_exception_events (exception_id, event_type, description, operator_id) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, exception_id, event_type, description, operator_id, created_at"
        )
        .bind(exception_id)
        .bind(event_type)
        .bind(description)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(ExceptionEvent::from_row(&row)?)
    }

    pub async fn list_events(
        executor: &mut sqlx::postgres::PgConnection,
        exception_id: i64,
    ) -> Result<Vec<ExceptionEvent>> {
        let rows = sqlx::query(
            "SELECT id, exception_id, event_type, description, operator_id, created_at \
             FROM production_exception_events \
             WHERE exception_id = $1 ORDER BY created_at DESC"
        )
        .bind(exception_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| ExceptionEvent::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }
}
