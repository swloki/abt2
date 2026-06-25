use rust_decimal::Decimal;
use sqlx::Postgres;

use super::model::*;
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::shared::types::{PgExecutor, Result};

pub struct PickListRepo;
pub struct PickListItemRepo;

const PICK_LIST_COLUMNS: &str = "id, doc_number, outbound_id, status, picker_id, picked_at, remark, operator_id, created_at, updated_at, deleted_at";
const PICK_LIST_ITEM_COLUMNS: &str = "id, pick_list_id, line_no, outbound_item_id, product_id, warehouse_id, bin_id, requested_qty, picked_qty, created_at";

impl PickListRepo {
    pub async fn insert(&self, executor: PgExecutor<'_>, params: &CreatePickListParams<'_>) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"INSERT INTO pick_lists (doc_number, outbound_id, status, picker_id, remark, operator_id)
               VALUES ($1, $2, 1, $3, $4, $5) RETURNING id"#,
        )
        .bind(params.doc_number)
        .bind(params.outbound_id)
        .bind(params.picker_id)
        .bind(params.remark)
        .bind(params.operator_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(id)
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<PickList>> {
        let sql = format!("SELECT {PICK_LIST_COLUMNS} FROM pick_lists WHERE id = $1 AND deleted_at IS NULL");
        let row = sqlx::query_as::<Postgres, PickList>(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(&mut *executor)
            .await?;
        Ok(row)
    }

    pub async fn find_by_outbound(&self, executor: PgExecutor<'_>, outbound_id: i64) -> Result<Option<PickList>> {
        let sql = format!(
            "SELECT {PICK_LIST_COLUMNS} FROM pick_lists WHERE outbound_id = $1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1"
        );
        let row = sqlx::query_as::<Postgres, PickList>(sqlx::AssertSqlSafe(sql))
            .bind(outbound_id)
            .fetch_optional(&mut *executor)
            .await?;
        Ok(row)
    }

    pub async fn mark_picked(&self, executor: PgExecutor<'_>, id: i64, picker_id: Option<i64>) -> Result<()> {
        sqlx::query(
            "UPDATE pick_lists SET status = 2, picker_id = COALESCE($2, picker_id), picked_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(picker_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    pub async fn update_status(&self, executor: PgExecutor<'_>, id: i64, status: PickListStatus) -> Result<()> {
        sqlx::query("UPDATE pick_lists SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(status)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    pub async fn list(
        &self,
        executor: PgExecutor<'_>,
        filter: &PickListQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<PickList>> {
        let offset = ((page.page.max(1) - 1) * page.page_size) as i64;
        let limit = page.page_size as i64;

        let items_sql = format!(
            "SELECT {PICK_LIST_COLUMNS} FROM pick_lists
             WHERE deleted_at IS NULL
               AND ($1::bigint IS NULL OR outbound_id = $1)
               AND ($2::int2 IS NULL OR status = $2)
             ORDER BY id DESC LIMIT $3 OFFSET $4"
        );
        let items = sqlx::query_as::<Postgres, PickList>(sqlx::AssertSqlSafe(items_sql))
            .bind(filter.outbound_id)
            .bind(filter.status.map(|s| s.as_i16()))
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *executor)
            .await?;

        let count_sql = "SELECT COUNT(*) FROM pick_lists WHERE deleted_at IS NULL AND ($1::bigint IS NULL OR outbound_id = $1) AND ($2::int2 IS NULL OR status = $2)";
        let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql.to_string()))
            .bind(filter.outbound_id)
            .bind(filter.status.map(|s| s.as_i16()))
            .fetch_one(&mut *executor)
            .await?;

        let total_pages = if page.page_size == 0 {
            0
        } else {
            (total as u64).div_ceil(page.page_size as u64) as u32
        };

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page: page.page,
            page_size: page.page_size,
            total_pages,
        })
    }
}

impl PickListItemRepo {
    pub async fn create_batch(
        &self,
        executor: PgExecutor<'_>,
        pick_list_id: i64,
        items: &[PickListItemInput],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"INSERT INTO pick_list_items
                   (pick_list_id, line_no, outbound_item_id, product_id, warehouse_id, bin_id, requested_qty, picked_qty)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            )
            .bind(pick_list_id)
            .bind(item.line_no)
            .bind(item.outbound_item_id)
            .bind(item.product_id)
            .bind(item.warehouse_id)
            .bind(item.bin_id)
            .bind(item.requested_qty)
            .bind(item.picked_qty)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_by_pick_list_id(&self, executor: PgExecutor<'_>, pick_list_id: i64) -> Result<Vec<PickListItem>> {
        let sql = format!(
            "SELECT {PICK_LIST_ITEM_COLUMNS} FROM pick_list_items WHERE pick_list_id = $1 ORDER BY line_no"
        );
        let rows = sqlx::query_as::<Postgres, PickListItem>(sqlx::AssertSqlSafe(sql))
            .bind(pick_list_id)
            .fetch_all(&mut *executor)
            .await?;
        Ok(rows)
    }

    /// 录入拣货结果：更新 picked_qty；warehouse_id/bin_id 传 None 时保留原值（COALESCE）。
    pub async fn update_picked(
        &self,
        executor: PgExecutor<'_>,
        pick_list_item_id: i64,
        picked_qty: Decimal,
        warehouse_id: Option<i64>,
        bin_id: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE pick_list_items SET picked_qty = $2, warehouse_id = COALESCE($3, warehouse_id), bin_id = COALESCE($4, bin_id) WHERE id = $1",
        )
        .bind(pick_list_item_id)
        .bind(picked_qty)
        .bind(warehouse_id)
        .bind(bin_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
}
