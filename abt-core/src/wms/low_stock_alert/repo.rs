use rust_decimal::Decimal;
use sqlx::postgres::PgConnection;
use sqlx::{FromRow, Row};

use super::model::{LowStockAlert, LowStockAlertFilter};
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::Result;
use crate::wms::enums::LowStockAlertStatus;

pub struct LowStockAlertRepo;

impl LowStockAlertRepo {
    /// 取产品×仓库的库存汇总（总量、安全库存总量）
    pub async fn stock_summary(
        executor: &mut PgConnection,
        product_id: i64,
        warehouse_id: i64,
    ) -> Result<(Decimal, Decimal)> {
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(quantity), 0)      AS qty,
                COALESCE(SUM(safety_stock), 0)  AS safety
            FROM stock_ledger
            WHERE product_id = $1 AND warehouse_id = $2
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .fetch_one(&mut *executor)
        .await?;

        let qty: Decimal = row.try_get("qty").unwrap_or(Decimal::ZERO);
        let safety: Decimal = row.try_get("safety").unwrap_or(Decimal::ZERO);
        Ok((qty, safety))
    }

    /// 是否已存在未确认的预警
    pub async fn has_active(
        executor: &mut PgConnection,
        product_id: i64,
        warehouse_id: i64,
    ) -> Result<bool> {
        let cnt: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM wms_low_stock_alerts
            WHERE product_id = $1 AND warehouse_id = $2 AND status = $3
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(LowStockAlertStatus::Active)
        .fetch_one(&mut *executor)
        .await?;
        Ok(cnt > 0)
    }

    pub async fn insert(
        executor: &mut PgConnection,
        product_id: i64,
        warehouse_id: i64,
        current_qty: Decimal,
        safety_stock: Decimal,
        operator_id: i64,
    ) -> Result<LowStockAlert> {
        let row = sqlx::query(
            r#"
            INSERT INTO wms_low_stock_alerts
                (product_id, warehouse_id, current_qty, safety_stock, status, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, product_id, warehouse_id, current_qty, safety_stock,
                      status, operator_id, created_at, acked_at
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(current_qty)
        .bind(safety_stock)
        .bind(LowStockAlertStatus::Active)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(LowStockAlert::from_row(&row)?)
    }

    /// 确认预警（Active → Acknowledged）
    pub async fn ack(executor: &mut PgConnection, id: i64) -> Result<u64> {
        let r = sqlx::query(
            "UPDATE wms_low_stock_alerts SET status = $1, acked_at = NOW() WHERE id = $2 AND status = $3",
        )
        .bind(LowStockAlertStatus::Acknowledged)
        .bind(id)
        .bind(LowStockAlertStatus::Active)
        .execute(&mut *executor)
        .await?;
        Ok(r.rows_affected())
    }

    pub async fn list(
        executor: &mut PgConnection,
        filter: &LowStockAlertFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<LowStockAlert>> {
        let offset = (page.saturating_sub(1)) * page_size;
        let mut where_clauses = vec!["1=1".to_string()];
        let mut idx = 0u32;
        if filter.status.is_some() {
            idx += 1;
            where_clauses.push(format!("status = ${idx}"));
        }
        if filter.warehouse_id.is_some() {
            idx += 1;
            where_clauses.push(format!("warehouse_id = ${idx}"));
        }
        let where_sql = where_clauses.join(" AND ");
        let limit_idx = idx + 1;
        let offset_idx = idx + 2;

        let count_sql = format!("SELECT COUNT(*) FROM wms_low_stock_alerts WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, product_id, warehouse_id, current_qty, safety_stock, status, \
             operator_id, created_at, acked_at \
             FROM wms_low_stock_alerts WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
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
        let items: Vec<LowStockAlert> = rows
            .iter()
            .filter_map(|r| LowStockAlert::from_row(r).ok())
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
