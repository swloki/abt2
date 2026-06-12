//! MES 需求池 — 数据库查询（基于视图 v_production_demands）

#![allow(deprecated)]

use rust_decimal::Decimal;
use sqlx::Row;

use crate::shared::types::{PgExecutor, Result};
use crate::shared::types::pagination::{PageParams, PaginatedResult};

use super::model::*;

pub struct MesDemandRepo;

impl MesDemandRepo {
    /// 查询视图 v_production_demands（封装跨模块 JOIN）
    /// 动态条件 + 分页
    pub async fn find_demands(
        db: PgExecutor<'_>,
        query: &DemandPoolQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<DemandSummary>> {
        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx: u32 = 1;

        let status_param;
        if let Some(s) = query.status {
            status_param = s;
            where_clauses.push(format!("demand_status = ${param_idx}"));
            param_idx += 1;
        } else {
            status_param = -1;
            where_clauses.push("demand_status = 1".to_string()); // 默认 Pending
        }

        let product_param;
        if let Some(pid) = query.product_id {
            product_param = pid;
            where_clauses.push(format!("product_id = ${param_idx}"));
            param_idx += 1;
        } else {
            product_param = -1;
        }

        let order_param;
        if let Some(oid) = query.order_id {
            order_param = oid;
            where_clauses.push(format!("order_id = ${param_idx}"));
            param_idx += 1;
        } else {
            order_param = -1;
        }

        let where_sql = where_clauses.join(" AND ");

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM v_production_demands WHERE {where_sql}");
        let mut count_q = sqlx::query(sqlx::AssertSqlSafe(count_sql));
        if query.status.is_some() { count_q = count_q.bind(status_param); }
        if query.product_id.is_some() { count_q = count_q.bind(product_param); }
        if query.order_id.is_some() { count_q = count_q.bind(order_param); }
        let count_row = count_q.fetch_one(&mut *db).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let offset = ((page.page.saturating_sub(1)) * page.page_size) as i64;
        let limit = page.page_size as i64;
        let data_sql = format!(
            "SELECT * FROM v_production_demands WHERE {where_sql} \
             ORDER BY required_date ASC NULLS LAST, priority DESC \
             LIMIT ${param_idx} OFFSET ${}",
            param_idx + 1
        );
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if query.status.is_some() { data_q = data_q.bind(status_param); }
        if query.product_id.is_some() { data_q = data_q.bind(product_param); }
        if query.order_id.is_some() { data_q = data_q.bind(order_param); }
        data_q = data_q.bind(limit).bind(offset);
        let rows = data_q.fetch_all(&mut *db).await?;

        let items: Vec<DemandSummary> = rows.iter().map(|r| DemandSummary {
            id: r.try_get("id").unwrap_or(0),
            order_id: r.try_get("order_id").unwrap_or(0),
            order_no: r.try_get("order_no").unwrap_or_default(),
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            quantity: r.try_get("quantity").unwrap_or(Decimal::ZERO),
            required_date: r.try_get("required_date").unwrap_or(None),
            priority: r.try_get("priority").unwrap_or(0),
            demand_status: r.try_get("demand_status").unwrap_or(0),
            created_at: r.try_get("created_at").unwrap_or(chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
        }).collect();

        Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
    }

    /// 按物料聚合查询（物料维度 — 计划员主要操作视图）
    pub async fn find_material_aggregated(
        db: PgExecutor<'_>,
        query: &MaterialAggQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>> {
        let mut where_clauses = vec!["demand_status = 1".to_string()]; // Pending only
        let mut param_idx: u32 = 1;

        let product_param;
        if let Some(pid) = query.product_id {
            product_param = pid;
            where_clauses.push(format!("product_id = ${param_idx}"));
            param_idx += 1;
        } else {
            product_param = -1;
        }

        let where_sql = where_clauses.join(" AND ");

        // Count
        let count_sql = format!(
            "SELECT COUNT(*) AS cnt FROM (
                SELECT product_id FROM v_production_demands WHERE {where_sql} GROUP BY product_id
             ) sub"
        );
        let mut count_q = sqlx::query(sqlx::AssertSqlSafe(count_sql));
        if query.product_id.is_some() { count_q = count_q.bind(product_param); }
        let count_row = count_q.fetch_one(&mut *db).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let offset = ((page.page.saturating_sub(1)) * page.page_size) as i64;
        let limit = page.page_size as i64;
        let data_sql = format!(
            "SELECT product_id, product_name, product_code, \
                    SUM(quantity) AS total_demand_qty, \
                    COUNT(*) AS demand_count, \
                    MIN(required_date) AS earliest_required_date, \
                    MAX(required_date) AS latest_required_date \
             FROM v_production_demands WHERE {where_sql} \
             GROUP BY product_id, product_name, product_code \
             ORDER BY total_demand_qty DESC \
             LIMIT ${param_idx} OFFSET ${}",
            param_idx + 1
        );
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if query.product_id.is_some() { data_q = data_q.bind(product_param); }
        data_q = data_q.bind(limit).bind(offset);
        let rows = data_q.fetch_all(&mut *db).await?;

        let items: Vec<MaterialAggSummary> = rows.iter().map(|r| MaterialAggSummary {
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            total_demand_qty: r.try_get("total_demand_qty").unwrap_or(Decimal::ZERO),
            demand_count: r.try_get("demand_count").unwrap_or(0),
            earliest_required_date: r.try_get("earliest_required_date").unwrap_or(None),
            latest_required_date: r.try_get("latest_required_date").unwrap_or(None),
        }).collect();

        Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
    }

    /// 乐观锁：批量锁定自制需求（原子 UPDATE + RETURNING）
    /// 只返回成功锁定的需求，未锁定的记入 skipped
    pub async fn lock_demands_for_production(
        db: PgExecutor<'_>,
        demand_ids: &[i64],
    ) -> Result<Vec<LockedDemand>> {
        let rows = sqlx::query_as::<_, LockedDemand>(
            r#"UPDATE demands SET status = 2, updated_at = NOW()
               WHERE id = ANY($1) AND status = 1 AND acquire_channel = 1 AND deleted_at IS NULL
               RETURNING id, product_id, source_id, source_line_id, acquire_channel, required_qty, required_date, priority"#,
        )
        .bind(demand_ids)
        .fetch_all(&mut *db)
        .await?;

        Ok(rows)
    }

    /// 按 ID 查询需求详情（从视图，供 Handler 使用）
    pub async fn find_detail_by_id(
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<DemandSummary>> {
        let row = sqlx::query(
            "SELECT * FROM v_production_demands WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *db)
        .await?;

        Ok(row.map(|r| DemandSummary {
            id: r.try_get("id").unwrap_or(0),
            order_id: r.try_get("order_id").unwrap_or(0),
            order_no: r.try_get("order_no").unwrap_or_default(),
            product_id: r.try_get("product_id").unwrap_or(0),
            product_name: r.try_get("product_name").unwrap_or_default(),
            product_code: r.try_get("product_code").unwrap_or_default(),
            quantity: r.try_get("quantity").unwrap_or(Decimal::ZERO),
            required_date: r.try_get("required_date").unwrap_or(None),
            priority: r.try_get("priority").unwrap_or(0),
            demand_status: r.try_get("demand_status").unwrap_or(0),
            created_at: r.try_get("created_at").unwrap_or(chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
        }))
    }
}
