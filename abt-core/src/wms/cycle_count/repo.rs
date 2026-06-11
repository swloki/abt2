use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::{CreateCycleCountReq, CycleCount, CycleCountFilter, CycleCountItem};
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::CycleCountStatus;

pub struct CycleCountRepo;

impl CycleCountRepo {
    /// 插入盘点单及其明细
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        req: &CreateCycleCountReq,
        operator_id: i64,
    ) -> Result<CycleCount> {
        let row = sqlx::query(
            r#"
            INSERT INTO cycle_counts
                (doc_number, warehouse_id, zone_id, count_date, status, is_blind, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, doc_number, warehouse_id, zone_id, count_date, status, is_blind,
                      remark, operator_id, created_at, updated_at
            "#,
        )
        .bind(doc_number)
        .bind(req.warehouse_id)
        .bind(req.zone_id)
        .bind(req.count_date)
        .bind(CycleCountStatus::Draft)
        .bind(req.is_blind)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        let count = CycleCount::from_row(&row)?;

        // 批量插入明细
        for item in &req.items {
            sqlx::query(
                r#"
                INSERT INTO cycle_count_items
                    (count_id, bin_id, product_id, batch_no, system_qty, counted_qty, variance_qty, is_adjusted)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(count.id)
            .bind(item.bin_id)
            .bind(item.product_id)
            .bind(&item.batch_no)
            .bind(item.system_qty)
            .bind(rust_decimal::Decimal::ZERO)
            .bind(rust_decimal::Decimal::ZERO)
            .bind(false)
            .execute(&mut *executor)
            .await?;
        }

        Ok(count)
    }

    /// 按 ID 查询盘点单
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<CycleCount>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, warehouse_id, zone_id, count_date, status, is_blind,
                   remark, operator_id, created_at, updated_at
            FROM cycle_counts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| CycleCount::from_row(&r).map_err(Into::into)).transpose()

    }

    /// 查询盘点单明细
    pub async fn get_items(
        executor: &mut sqlx::postgres::PgConnection,
        count_id: i64,
    ) -> Result<Vec<CycleCountItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, count_id, bin_id, product_id, batch_no, system_qty, counted_qty,
                   variance_qty, variance_reason, is_adjusted
            FROM cycle_count_items
            WHERE count_id = $1
            ORDER BY id
            "#,
        )
        .bind(count_id)
        .fetch_all(&mut *executor)
        .await?;

        Ok(rows
            .iter()
            .filter_map(|r| CycleCountItem::from_row(r).ok())
            .collect::<Vec<_>>())
    }

    /// 更新盘点单状态
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: CycleCountStatus,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE cycle_counts SET status = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(status)
        .bind(id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 更新明细的盘点数量和差异数量
    pub async fn update_item_counted(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        counted_qty: rust_decimal::Decimal,
        variance_qty: rust_decimal::Decimal,
        variance_reason: Option<&str>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE cycle_count_items SET counted_qty = $1, variance_qty = $2, variance_reason = $3 WHERE id = $4",
        )
        .bind(counted_qty)
        .bind(variance_qty)
        .bind(variance_reason)
        .bind(item_id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 标记所有明细为已调整
    pub async fn mark_items_adjusted(
        executor: &mut sqlx::postgres::PgConnection,
        count_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE cycle_count_items SET is_adjusted = true WHERE count_id = $1",
        )
        .bind(count_id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 分页查询盘点单
    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &CycleCountFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<CycleCount>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("cc.status = ${param_idx}"));
        }
        if filter.warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("cc.warehouse_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM cycle_counts cc WHERE {where_sql}");
        let data_sql = format!(
            "SELECT cc.id, cc.doc_number, cc.warehouse_id, cc.zone_id, cc.count_date, cc.status, cc.is_blind, \
             cc.remark, cc.operator_id, cc.created_at, cc.updated_at, \
             (SELECT COUNT(*) FROM cycle_count_items cci WHERE cci.count_id = cc.id) AS item_count \
             FROM cycle_counts cc WHERE {where_sql} \
             ORDER BY cc.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.warehouse_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i32).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<CycleCount> = rows
            .iter()
            .filter_map(|r| CycleCount::from_row(r).ok())
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
