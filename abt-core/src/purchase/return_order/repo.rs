use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;

use super::model::{
    CreatePurchaseReturnRequest, CreateReturnItemRequest, PurchaseReturn, PurchaseReturnItem,
    PurchaseReturnQuery,
};
use crate::purchase::enums::PurchaseReturnStatus;
use crate::shared::types::pagination::PageParams;

pub struct PurchaseReturnRepo;

impl PurchaseReturnRepo {
    /// INSERT 采购退货主表，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreatePurchaseReturnRequest,
        doc_number: &str,
        total_amount: Decimal,
        operator_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO purchase_returns
                (doc_number, order_id, supplier_id, return_date, status,
                 return_reason, total_amount, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(doc_number)
        .bind(req.order_id)
        .bind(req.supplier_id)
        .bind(req.return_date)
        .bind(PurchaseReturnStatus::Draft)
        .bind(&req.return_reason)
        .bind(total_amount)
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
    ) -> Result<Option<PurchaseReturn>, sqlx::Error> {
        sqlx::query_as::<_, PurchaseReturn>(
            r#"
            SELECT id, doc_number, order_id, supplier_id, return_date, status,
                   return_reason, total_amount, remark, operator_id,
                   created_at, updated_at, deleted_at
            FROM purchase_returns
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
        q: &PurchaseReturnQuery,
        page: &PageParams,
    ) -> Result<(Vec<PurchaseReturn>, u64), sqlx::Error> {
        let where_clause = "
            WHERE deleted_at IS NULL
              AND ($1::bigint IS NULL OR order_id = $1)
              AND ($2::bigint IS NULL OR supplier_id = $2)
              AND ($3::smallint IS NULL OR status = $3)
        ";

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM purchase_returns {where_clause}");
        let count_row = sqlx::query(&count_sql)
            .bind(q.order_id)
            .bind(q.supplier_id)
            .bind(q.status)
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT id, doc_number, order_id, supplier_id, return_date, status,
                    return_reason, total_amount, remark, operator_id,
                    created_at, updated_at, deleted_at
             FROM purchase_returns {where_clause}
             ORDER BY created_at DESC
             LIMIT $4 OFFSET $5"
        );
        let rows = sqlx::query_as::<_, PurchaseReturn>(&data_sql)
            .bind(q.order_id)
            .bind(q.supplier_id)
            .bind(q.status)
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
        status: PurchaseReturnStatus,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE purchase_returns
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

    /// 按供应商和订单列表查询已发货（Shipped）状态的退货单
    pub async fn list_shipped_by_supplier_for_orders(
        executor: &mut sqlx::postgres::PgConnection,
        supplier_id: i64,
        order_ids: &[i64],
    ) -> Result<Vec<PurchaseReturn>, sqlx::Error> {
        if order_ids.is_empty() {
            return Ok(vec![]);
        }
        // 动态构建参数化 IN 子句
        let placeholders: Vec<String> = (0..order_ids.len()).map(|i| format!("${}", i + 3)).collect();
        let sql = format!(
            r#"
            SELECT id, doc_number, order_id, supplier_id, return_date, status,
                   return_reason, total_amount, remark, operator_id,
                   created_at, updated_at, deleted_at
            FROM purchase_returns
            WHERE supplier_id = $1
              AND status = $2
              AND order_id IN ({})
              AND deleted_at IS NULL
            ORDER BY return_date
            "#,
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, PurchaseReturn>(&sql)
            .bind(supplier_id)
            .bind(PurchaseReturnStatus::Shipped);
        for &oid in order_ids {
            query = query.bind(oid);
        }
        query.fetch_all(executor).await
    }
}

// ---------------------------------------------------------------------------
// PurchaseReturnItemRepo
// ---------------------------------------------------------------------------

pub struct PurchaseReturnItemRepo;

impl PurchaseReturnItemRepo {
    /// 批量 INSERT 退货明细
    pub async fn insert_items(
        executor: &mut sqlx::postgres::PgConnection,
        return_id: i64,
        items: &[CreateReturnItemRequest],
    ) -> Result<(), sqlx::Error> {
        for item in items {
            let amount = item.returned_qty * item.unit_price;
            sqlx::query(
                r#"
                INSERT INTO purchase_return_items
                    (return_id, order_item_id, product_id, returned_qty, unit_price, amount)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(return_id)
            .bind(item.order_item_id)
            .bind(item.product_id)
            .bind(item.returned_qty)
            .bind(item.unit_price)
            .bind(amount)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按退货主表 id 查询全部明细
    pub async fn list_by_return_id(
        executor: &mut sqlx::postgres::PgConnection,
        return_id: i64,
    ) -> Result<Vec<PurchaseReturnItem>, sqlx::Error> {
        sqlx::query_as::<_, PurchaseReturnItem>(
            r#"
            SELECT id, return_id, order_item_id, product_id, returned_qty, unit_price, amount
            FROM purchase_return_items
            WHERE return_id = $1
            "#,
        )
        .bind(return_id)
        .fetch_all(executor)
        .await
    }
}
