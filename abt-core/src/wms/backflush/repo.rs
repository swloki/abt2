use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::{BackflushFilter, BackflushItem, BackflushRecord, CreateBackflushItemReq, CreateBackflushReq};
use crate::shared::types::pagination::PaginatedResult;

pub struct BackflushRepo;

impl BackflushRepo {
    /// 插入冲扣记录
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateBackflushReq,
    ) -> Result<BackflushRecord> {
        let row = sqlx::query(
            r#"
            INSERT INTO backflush_records
                (doc_number, work_order_id, product_id, completed_qty,
                 backflush_date, status, variance_threshold, operator_id,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            RETURNING id, doc_number, work_order_id, product_id, completed_qty,
                      backflush_date, status, variance_threshold, operator_id,
                      created_at, updated_at
            "#,
        )
        .bind(&req.doc_number)
        .bind(req.work_order_id)
        .bind(req.product_id)
        .bind(req.completed_qty)
        .bind(req.backflush_date)
        .bind(super::super::enums::BackflushStatus::Draft)
        .bind(req.variance_threshold)
        .bind(req.operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(BackflushRecord::from_row(&row)?)
    }

    /// 插入冲扣明细
    pub async fn insert_item(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateBackflushItemReq,
    ) -> Result<BackflushItem> {
        let row = sqlx::query(
            r#"
            INSERT INTO backflush_items
                (record_id, component_id, theoretical_qty, actual_qty,
                 variance_qty, variance_rate, is_over_threshold)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, record_id, component_id, theoretical_qty, actual_qty,
                      variance_qty, variance_rate, is_over_threshold
            "#,
        )
        .bind(req.record_id)
        .bind(req.component_id)
        .bind(req.theoretical_qty)
        .bind(req.actual_qty)
        .bind(req.variance_qty)
        .bind(req.variance_rate)
        .bind(req.is_over_threshold)
        .fetch_one(&mut *executor)
        .await?;

        Ok(BackflushItem::from_row(&row)?)
    }

    /// 按 ID 查询冲扣记录
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<BackflushRecord>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, product_id, completed_qty,
                   backflush_date, status, variance_threshold, operator_id,
                   created_at, updated_at
            FROM backflush_records
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| BackflushRecord::from_row(&r).map_err(Into::into)).transpose()

    }

    /// 查询冲扣记录的所有明细
    pub async fn get_items(
        executor: &mut sqlx::postgres::PgConnection,
        record_id: i64,
    ) -> Result<Vec<BackflushItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, record_id, component_id, theoretical_qty, actual_qty,
                   variance_qty, variance_rate, is_over_threshold
            FROM backflush_items
            WHERE record_id = $1
            ORDER BY id
            "#,
        )
        .bind(record_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| BackflushItem::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    /// 更新冲扣记录状态
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: super::super::enums::BackflushStatus,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE backflush_records
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 分页查询冲扣记录
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &BackflushFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<BackflushRecord>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("br.status = ${param_idx}"));
        }
        if filter.work_order_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("br.work_order_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM backflush_records WHERE {where_sql}");
        let data_sql = format!(
            "SELECT br.id, br.doc_number, br.work_order_id, br.product_id, br.completed_qty, \
             br.backflush_date, br.status, br.variance_threshold, br.operator_id, \
             br.created_at, br.updated_at, \
             (SELECT EXISTS(SELECT 1 FROM backflush_items bi WHERE bi.record_id = br.id AND bi.is_over_threshold = true)) AS has_variance_warning \
             FROM backflush_records br WHERE {where_sql} \
             ORDER BY br.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.work_order_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<BackflushRecord> = rows
            .iter()
            .filter_map(|r| BackflushRecord::from_row(r).ok())
            .collect();

        let total_pages = (total as u64).div_ceil(page_size as u64) as u32;

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }
}
