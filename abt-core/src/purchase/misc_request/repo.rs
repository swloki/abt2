use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;

use super::model::{
    CreateMiscItemRequest, CreateMiscRequestRequest, MiscRequestItem, MiscRequestQuery,
    MiscellaneousRequest,
};
use crate::purchase::enums::MiscRequestStatus;
use crate::shared::types::pagination::PageParams;

pub struct MiscRequestRepo;

impl MiscRequestRepo {
    /// INSERT 零星请购主表，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateMiscRequestRequest,
        doc_number: &str,
        total_amount: Decimal,
        operator_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO misc_requests
                (doc_number, department_id, request_date, status, total_amount,
                 purpose, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(doc_number)
        .bind(req.department_id)
        .bind(req.request_date)
        .bind(MiscRequestStatus::Draft)
        .bind(total_amount)
        .bind(&req.purpose)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        row.try_get("id")
    }

    /// 按主键查询（软删除行过滤）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<MiscellaneousRequest>, sqlx::Error> {
        sqlx::query_as::<_, MiscellaneousRequest>(
            r#"
            SELECT id, doc_number, department_id, request_date, status, total_amount,
                   purpose, remark, operator_id, created_at, updated_at, deleted_at
            FROM misc_requests
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
    }

    /// 动态条件分页查询
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &MiscRequestQuery,
        page: &PageParams,
    ) -> Result<(Vec<MiscellaneousRequest>, u64), sqlx::Error> {
        let where_clause = "
            WHERE deleted_at IS NULL
              AND ($1::bigint IS NULL OR department_id = $1)
              AND ($2::smallint IS NULL OR status = $2)
              AND ($3::date IS NULL OR request_date >= $3)
              AND ($4::date IS NULL OR request_date <= $4)
        ";

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM misc_requests {where_clause}");
        let count_row = sqlx::query(&count_sql)
            .bind(q.department_id)
            .bind(q.status)
            .bind(q.request_date_start)
            .bind(q.request_date_end)
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT id, doc_number, department_id, request_date, status, total_amount,
                    purpose, remark, operator_id, created_at, updated_at, deleted_at
             FROM misc_requests {where_clause}
             ORDER BY created_at DESC
             LIMIT $5 OFFSET $6"
        );
        let rows = sqlx::query_as::<_, MiscellaneousRequest>(&data_sql)
            .bind(q.department_id)
            .bind(q.status)
            .bind(q.request_date_start)
            .bind(q.request_date_end)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *executor)
            .await?;

        Ok((rows, total as u64))
    }

    /// 状态变更（乐观锁：WHERE updated_at = $2）
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: MiscRequestStatus,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE misc_requests
            SET status = $1, updated_at = NOW()
            WHERE id = $2 AND updated_at = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(status)
        .bind(id)
        .bind(updated_at)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }
}

// ---------------------------------------------------------------------------
// MiscRequestItemRepo
// ---------------------------------------------------------------------------

pub struct MiscRequestItemRepo;

impl MiscRequestItemRepo {
    /// 批量 INSERT 零星请购明细
    pub async fn insert_items(
        executor: &mut sqlx::postgres::PgConnection,
        request_id: i64,
        items: &[CreateMiscItemRequest],
    ) -> Result<(), sqlx::Error> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO misc_request_items
                    (request_id, line_no, item_name, specification, quantity,
                     unit, estimated_price, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(request_id)
            .bind(item.line_no)
            .bind(&item.item_name)
            .bind(&item.specification)
            .bind(item.quantity)
            .bind(&item.unit)
            .bind(item.estimated_price)
            .bind(&item.remark)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按零星请购主表 id 查询全部明细
    pub async fn list_by_request_id(
        executor: &mut sqlx::postgres::PgConnection,
        request_id: i64,
    ) -> Result<Vec<MiscRequestItem>, sqlx::Error> {
        sqlx::query_as::<_, MiscRequestItem>(
            r#"
            SELECT id, request_id, line_no, item_name, specification, quantity,
                   unit, estimated_price, remark
            FROM misc_request_items
            WHERE request_id = $1
            ORDER BY line_no
            "#,
        )
        .bind(request_id)
        .fetch_all(executor)
        .await
    }
}
