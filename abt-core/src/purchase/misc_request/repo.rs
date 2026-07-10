use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use crate::shared::types::Result;

use super::model::{
    CreateMiscItemRequest, CreateMiscRequestRequest, MiscRequestItem, MiscRequestQuery,
    MiscellaneousRequest,
};
use crate::purchase::enums::MiscRequestStatus;
use crate::shared::types::pagination::{DataScope, PageParams};

pub struct MiscRequestRepo;

impl MiscRequestRepo {
    /// INSERT 零星请购主表，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateMiscRequestRequest,
        doc_number: &str,
        total_amount: Decimal,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO miscellaneous_requests
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

        Ok(row.try_get("id")?)
    }

    /// 按主键查询（软删除行过滤）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<MiscellaneousRequest>> {
        sqlx::query_as::<_, MiscellaneousRequest>(
            r#"
            SELECT id, doc_number, department_id, request_date, status, total_amount,
                   purpose, remark, operator_id, created_at, updated_at, deleted_at
            FROM miscellaneous_requests
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await.map_err(Into::into)
    }

    /// 动态条件分页查询（支持 DataScope 行级权限过滤）
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &MiscRequestQuery,
        page: &PageParams,
        scope: (DataScope, i64, Option<i64>),
    ) -> Result<(Vec<MiscellaneousRequest>, u64)> {
        let (data_scope, operator_id, department_id) = scope;
        // miscellaneous_requests 有 department_id，可按部门过滤
        // 占位符：$1 department_id, $2 status, $3 date_start, $4 date_end, $5 doc_number, $6 item_keyword
        //         count scope $7；data LIMIT $7 OFFSET $8 + scope $9
        let (count_scope_clause, data_scope_clause) = match data_scope {
            DataScope::All => ("", ""),
            DataScope::Department => ("AND department_id = $7", "AND department_id = $9"),
            DataScope::SelfOnly => ("AND operator_id = $7", "AND operator_id = $9"),
        };
        let scope_bind_id = match data_scope {
            DataScope::Department => department_id.unwrap_or(operator_id),
            _ => operator_id,
        };
        // 排序：白名单列名 + 方向（防注入）。misc 无供应商概念，不支持 supplier 排序
        let (order_col, default_asc) = match q.sort.as_deref() {
            Some("amount") => ("total_amount", false),
            Some("purpose") => ("purpose", true),
            Some("doc") => ("doc_number", false),
            _ => ("request_date", false), // date 或默认
        };
        let asc = match q.dir.as_deref() {
            Some("asc") => true,
            Some("desc") => false,
            _ => default_asc,
        };
        let order_clause = format!("{order_col} {}", if asc { "ASC" } else { "DESC" });
        let where_base = "WHERE deleted_at IS NULL
              AND ($1::bigint IS NULL OR department_id = $1)
              AND ($2::smallint IS NULL OR status = $2)
              AND ($3::date IS NULL OR request_date >= $3)
              AND ($4::date IS NULL OR request_date <= $4)
              AND ($5::text IS NULL OR doc_number ILIKE '%' || $5 || '%')
              AND ($6::text IS NULL OR EXISTS (
                    SELECT 1 FROM misc_request_items mi
                    WHERE mi.request_id = miscellaneous_requests.id
                      AND (mi.item_name ILIKE '%' || $6 || '%'
                           OR mi.specification ILIKE '%' || $6 || '%')))";
        let count_where = format!("{where_base} {count_scope_clause}");
        let data_where = format!("{where_base} {data_scope_clause}");

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM miscellaneous_requests {count_where}");
        let mut count_query = sqlx::query(sqlx::AssertSqlSafe(count_sql))
            .bind(q.department_id)
            .bind(q.status)
            .bind(q.request_date_start)
            .bind(q.request_date_end)
            .bind(q.doc_number.as_deref())
            .bind(q.item_keyword.as_deref());
        if !matches!(data_scope, DataScope::All) {
            count_query = count_query.bind(scope_bind_id);
        }
        let count_row = count_query.fetch_one(&mut *executor).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT id, doc_number, department_id, request_date, status, total_amount,
                    purpose, remark, operator_id, created_at, updated_at, deleted_at
             FROM miscellaneous_requests {data_where}
             ORDER BY {order_clause}
             LIMIT $7 OFFSET $8"
        );
        let mut data_query = sqlx::query_as::<_, MiscellaneousRequest>(sqlx::AssertSqlSafe(data_sql))
            .bind(q.department_id)
            .bind(q.status)
            .bind(q.request_date_start)
            .bind(q.request_date_end)
            .bind(q.doc_number.as_deref())
            .bind(q.item_keyword.as_deref())
            .bind(limit)
            .bind(offset);
        if !matches!(data_scope, DataScope::All) {
            data_query = data_query.bind(scope_bind_id);
        }
        let rows = data_query.fetch_all(&mut *executor).await?;

        Ok((rows, total as u64))
    }

    /// 状态变更（乐观锁：WHERE updated_at = $2）
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: MiscRequestStatus,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE miscellaneous_requests
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
    ) -> Result<()> {
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
    ) -> Result<Vec<MiscRequestItem>> {
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
        .await.map_err(Into::into)
    }

    /// 批量查多个零星请购的明细（避免逐个 list_by_request_id 的 N+1）；结果含 request_id，调用方按需分组。
    pub async fn list_by_request_ids(
        executor: &mut sqlx::postgres::PgConnection,
        request_ids: &[i64],
    ) -> Result<Vec<MiscRequestItem>> {
        if request_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, MiscRequestItem>(
            r#"
            SELECT id, request_id, line_no, item_name, specification, quantity,
                   unit, estimated_price, remark
            FROM misc_request_items
            WHERE request_id = ANY($1)
            ORDER BY line_no
            "#,
        )
        .bind(request_ids)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }
}
