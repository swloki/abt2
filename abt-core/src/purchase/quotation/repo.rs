use chrono::{DateTime, Utc};
use sqlx::Row;
use crate::shared::types::Result;

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
                (doc_number, supplier_id, quotation_date, valid_from, valid_until, status, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
                   status, remark, operator_id, created_at, updated_at, deleted_at
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
        let scope_clause = match data_scope {
            DataScope::All => "",
            _ => "AND operator_id = $7",
        };
        let where_clause = format!(
            "WHERE deleted_at IS NULL
              AND ($1::bigint IS NULL OR supplier_id = $1)
              AND ($2::smallint IS NULL OR status = $2)
              AND ($3::date IS NULL OR quotation_date >= $3)
              AND ($4::date IS NULL OR quotation_date <= $4)
              {scope_clause}"
        );

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM purchase_quotations {where_clause}");
        let mut count_query = sqlx::query(&count_sql)
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.quotation_date_start)
            .bind(q.quotation_date_end);
        if !matches!(data_scope, DataScope::All) {
            count_query = count_query.bind(operator_id);
        }
        let count_row = count_query.fetch_one(&mut *executor).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT id, doc_number, supplier_id, quotation_date, valid_from, valid_until,
                    status, remark, operator_id, created_at, updated_at, deleted_at
             FROM purchase_quotations {where_clause}
             ORDER BY created_at DESC
             LIMIT $5 OFFSET $6"
        );
        let mut data_query = sqlx::query_as::<_, PurchaseQuotation>(&data_sql)
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.quotation_date_start)
            .bind(q.quotation_date_end)
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
}
