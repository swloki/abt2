use chrono::{DateTime, Utc};
use sqlx::Row;
use crate::shared::types::{PgExecutor, Result};

use super::model::{
    CreateQuotationItemRequest, CreatePurchaseQuotationRequest, PurchaseQuotation,
    PurchaseQuotationItem, PurchaseQuotationQuery, QuotationComparison,
};
use crate::purchase::enums::PurchaseQuotationStatus;
use crate::shared::types::pagination::{DataScope, PageParams};

pub struct PurchaseQuotationRepo;

impl PurchaseQuotationRepo {
    /// INSERT 采购报价主表，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreatePurchaseQuotationRequest,
        doc_number: &str,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO purchase_quotations
                (doc_number, supplier_id, quotation_date, valid_from, valid_until, status, remark, operator_id,
                 currency, buyer_id, supplier_quotation_no)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#,
        )
        .bind(doc_number)
        .bind(req.supplier_id)
        .bind(req.quotation_date)
        .bind(req.valid_from)
        .bind(req.valid_until)
        .bind(PurchaseQuotationStatus::Draft)
        .bind(&req.remark)
        .bind(operator_id)
        .bind(&req.currency)
        .bind(req.buyer_id)
        .bind(&req.supplier_quotation_no)
        .fetch_one(executor)
        .await?;

        Ok(row.try_get("id")?)
    }

    /// 按主键查询（软删除行过滤）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<PurchaseQuotation>> {
        sqlx::query_as::<_, PurchaseQuotation>(
            r#"
            SELECT id, doc_number, supplier_id, quotation_date, valid_from, valid_until,
                   status, remark, operator_id, currency, buyer_id, supplier_quotation_no,
                   created_at, updated_at, deleted_at
            FROM purchase_quotations
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await.map_err(Into::into)
    }

    /// 通过报价明细 id 反查关联的报价单
    pub async fn get_by_item_id(
        executor: &mut sqlx::postgres::PgConnection,
        quotation_item_id: i64,
    ) -> Result<Option<PurchaseQuotation>> {
        sqlx::query_as::<_, PurchaseQuotation>(
            r#"
            SELECT q.id, q.doc_number, q.supplier_id, q.quotation_date, q.valid_from,
                   q.valid_until, q.status, q.remark, q.operator_id,
                   q.currency, q.buyer_id, q.supplier_quotation_no,
                   q.created_at, q.updated_at, q.deleted_at
            FROM purchase_quotations q
            JOIN purchase_quotation_items qi ON qi.quotation_id = q.id
            WHERE qi.id = $1 AND q.deleted_at IS NULL
            "#,
        )
        .bind(quotation_item_id)
        .fetch_optional(executor)
        .await.map_err(Into::into)
    }

    /// 动态条件分页查询（支持 DataScope 行级权限过滤）
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &PurchaseQuotationQuery,
        page: &PageParams,
        scope: (DataScope, i64, Option<i64>),
    ) -> Result<(Vec<PurchaseQuotation>, u64)> {
        let (data_scope, operator_id, _department_id) = scope;
        // purchase_quotations 无 department_id，Department 降级为 SelfOnly
        // 占位符：$1 supplier_id, $2 status, $3 date_start, $4 date_end, $5 doc_number, $6 product_keyword
        //         count scope $7；data LIMIT $7 OFFSET $8 + scope $9
        let count_scope_clause = if matches!(data_scope, DataScope::All) {
            ""
        } else {
            "AND purchase_quotations.operator_id = $7"
        };
        let data_scope_clause = if matches!(data_scope, DataScope::All) {
            ""
        } else {
            "AND purchase_quotations.operator_id = $9"
        };
        // 排序：白名单列名 + 方向（防注入）。sort=supplier 需 LEFT JOIN suppliers
        let (order_col, default_asc, need_join) = match q.sort.as_deref() {
            Some("valid") => ("valid_from", false, false),
            Some("supplier") => ("s.supplier_name", true, true),
            Some("doc") => ("doc_number", false, false),
            _ => ("quotation_date", false, false), // date 或默认
        };
        let asc = match q.dir.as_deref() {
            Some("asc") => true,
            Some("desc") => false,
            _ => default_asc,
        };
        let order_clause = format!(
            "{order_col} {}{}",
            if asc { "ASC" } else { "DESC" },
            if need_join { " NULLS LAST" } else { "" }
        );
        let join_clause = if need_join {
            "LEFT JOIN suppliers s ON s.supplier_id = purchase_quotations.supplier_id AND s.deleted_at IS NULL"
        } else {
            ""
        };
        let where_base = "WHERE purchase_quotations.deleted_at IS NULL
              AND ($1::bigint IS NULL OR purchase_quotations.supplier_id = $1)
              AND ($2::smallint IS NULL OR purchase_quotations.status = $2)
              AND ($3::date IS NULL OR purchase_quotations.quotation_date >= $3)
              AND ($4::date IS NULL OR purchase_quotations.quotation_date <= $4)
              AND ($5::text IS NULL OR purchase_quotations.doc_number ILIKE '%' || $5 || '%')
              AND ($6::text IS NULL OR EXISTS (
                    SELECT 1 FROM purchase_quotation_items qi
                    JOIN products p ON p.product_id = qi.product_id AND p.deleted_at IS NULL
                    WHERE qi.quotation_id = purchase_quotations.id
                      AND (p.product_code ILIKE '%' || $6 || '%'
                           OR p.pdt_name ILIKE '%' || $6 || '%')))";
        let count_where = format!("{where_base} {count_scope_clause}");
        let data_where = format!("{where_base} {data_scope_clause}");

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM purchase_quotations {count_where}");
        let mut count_query = sqlx::query(sqlx::AssertSqlSafe(count_sql))
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.quotation_date_start)
            .bind(q.quotation_date_end)
            .bind(q.doc_number.as_deref())
            .bind(q.product_keyword.as_deref());
        if !matches!(data_scope, DataScope::All) {
            count_query = count_query.bind(operator_id);
        }
        let count_row = count_query.fetch_one(&mut *executor).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT purchase_quotations.id, purchase_quotations.doc_number, purchase_quotations.supplier_id, purchase_quotations.quotation_date, purchase_quotations.valid_from, purchase_quotations.valid_until,
                    purchase_quotations.status, purchase_quotations.remark, purchase_quotations.operator_id, purchase_quotations.currency, purchase_quotations.buyer_id, purchase_quotations.supplier_quotation_no,
                    purchase_quotations.created_at, purchase_quotations.updated_at, purchase_quotations.deleted_at
             FROM purchase_quotations {join_clause} {data_where}
             ORDER BY {order_clause}
             LIMIT $7 OFFSET $8"
        );
        let mut data_query = sqlx::query_as::<_, PurchaseQuotation>(sqlx::AssertSqlSafe(data_sql))
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.quotation_date_start)
            .bind(q.quotation_date_end)
            .bind(q.doc_number.as_deref())
            .bind(q.product_keyword.as_deref())
            .bind(limit)
            .bind(offset);
        if !matches!(data_scope, DataScope::All) {
            data_query = data_query.bind(operator_id);
        }
        let rows = data_query.fetch_all(&mut *executor).await?;

        Ok((rows, total as u64))
    }

    /// 状态变更（乐观锁：WHERE updated_at = $2）
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: PurchaseQuotationStatus,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE purchase_quotations
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

    /// 按产品维度查询供应商报价对比
    pub async fn compare_by_product(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
    ) -> Result<Vec<QuotationComparison>> {
        let rows = sqlx::query(
            r#"
            SELECT qi.product_id, q.supplier_id, qi.unit_price, qi.currency,
                   q.valid_until, qi.is_preferred
            FROM purchase_quotation_items qi
            JOIN purchase_quotations q ON q.id = qi.quotation_id
            WHERE qi.product_id = $1
              AND q.status = $2
              AND q.deleted_at IS NULL
              AND q.valid_until >= CURRENT_DATE
            ORDER BY qi.unit_price ASC
            "#,
        )
        .bind(product_id)
        .bind(PurchaseQuotationStatus::Active)
        .fetch_all(&mut *executor)
        .await?;

        let items: Vec<QuotationComparison> = rows
            .iter()
            .map(|r| QuotationComparison {
                product_id: r.try_get("product_id").unwrap(),
                supplier_id: r.try_get("supplier_id").unwrap(),
                unit_price: r.try_get("unit_price").unwrap(),
                currency: r.try_get("currency").unwrap(),
                valid_until: r.try_get("valid_until").unwrap(),
                is_preferred: r.try_get("is_preferred").unwrap(),
            })
            .collect();

        Ok(items)
    }

    /// 软删除报价单
    pub async fn soft_delete(executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE purchase_quotations SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    /// 懒过期：把已过有效期的 Active 报价置为 Expired（幂等，无返回值；list 前调用）
    pub async fn expire_overdue(executor: PgExecutor<'_>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE purchase_quotations
            SET status = $1, updated_at = NOW()
            WHERE status = $2 AND valid_until < CURRENT_DATE AND deleted_at IS NULL
            "#,
        )
        .bind(PurchaseQuotationStatus::Expired)
        .bind(PurchaseQuotationStatus::Active)
        .execute(executor)
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PurchaseQuotationItemRepo
// ---------------------------------------------------------------------------

pub struct PurchaseQuotationItemRepo;

impl PurchaseQuotationItemRepo {
    /// 批量 INSERT 报价明细
    pub async fn insert_items(
        executor: &mut sqlx::postgres::PgConnection,
        quotation_id: i64,
        items: &[CreateQuotationItemRequest],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO purchase_quotation_items
                    (quotation_id, product_id, line_no, unit_price, min_order_qty,
                     lead_time_days, currency, is_preferred)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(quotation_id)
            .bind(item.product_id)
            .bind(item.line_no)
            .bind(item.unit_price)
            .bind(item.min_order_qty)
            .bind(item.lead_time_days)
            .bind(&item.currency)
            .bind(item.is_preferred)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按报价主表 id 查询全部明细
    pub async fn list_by_quotation_id(
        executor: &mut sqlx::postgres::PgConnection,
        quotation_id: i64,
    ) -> Result<Vec<PurchaseQuotationItem>> {
        sqlx::query_as::<_, PurchaseQuotationItem>(
            r#"
            SELECT id, quotation_id, product_id, line_no, unit_price, min_order_qty,
                   lead_time_days, currency, is_preferred
            FROM purchase_quotation_items
            WHERE quotation_id = $1
            ORDER BY line_no
            "#,
        )
        .bind(quotation_id)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }

    /// 批量查多个报价的明细（避免逐个 list_by_quotation_id 的 N+1）；结果含 quotation_id，调用方按需分组。
    pub async fn list_by_quotation_ids(
        executor: &mut sqlx::postgres::PgConnection,
        quotation_ids: &[i64],
    ) -> Result<Vec<PurchaseQuotationItem>> {
        if quotation_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, PurchaseQuotationItem>(
            r#"
            SELECT id, quotation_id, product_id, line_no, unit_price, min_order_qty,
                   lead_time_days, currency, is_preferred
            FROM purchase_quotation_items
            WHERE quotation_id = ANY($1)
            ORDER BY line_no
            "#,
        )
        .bind(quotation_ids)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }
}
