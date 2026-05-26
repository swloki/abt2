use sqlx::FromRow;
use crate::shared::types::RepoResult;

use super::model::{CreateLockReq, InventoryLock, LockFilter};
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::LockStatus;

pub struct InventoryLockRepo;

impl InventoryLockRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        req: &CreateLockReq,
        operator_id: i64,
    ) -> RepoResult<InventoryLock> {
        let row = sqlx::query(
            r#"
            INSERT INTO inventory_locks
                (doc_number, product_id, warehouse_id, locked_qty, lock_reason,
                 customer_id, status, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, doc_number, product_id, warehouse_id, locked_qty, lock_reason,
                      customer_id, status, operator_id, created_at, updated_at
            "#,
        )
        .bind(doc_number)
        .bind(req.product_id)
        .bind(req.warehouse_id)
        .bind(req.locked_qty)
        .bind(&req.lock_reason)
        .bind(req.customer_id)
        .bind(LockStatus::Active)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(InventoryLock::from_row(&row)?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> RepoResult<Option<InventoryLock>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, product_id, warehouse_id, locked_qty, lock_reason,
                   customer_id, status, operator_id, created_at, updated_at
            FROM inventory_locks
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| InventoryLock::from_row(&r).map_err(Into::into)).transpose()

    }

    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: LockStatus,
    ) -> RepoResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_locks
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

    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &LockFilter,
        page: u32,
        page_size: u32,
    ) -> RepoResult<PaginatedResult<InventoryLock>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }
        if filter.product_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("product_id = ${param_idx}"));
        }
        if filter.warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("warehouse_id = ${param_idx}"));
        }
        if filter.customer_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("customer_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM inventory_locks WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, doc_number, product_id, warehouse_id, locked_qty, lock_reason, \
             customer_id, status, operator_id, created_at, updated_at \
             FROM inventory_locks WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.product_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.warehouse_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.customer_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<InventoryLock> = rows
            .iter()
            .filter_map(|r| InventoryLock::from_row(r).ok())
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
