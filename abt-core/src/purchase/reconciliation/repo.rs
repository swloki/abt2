use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use crate::shared::types::Result;

use super::model::{PurchaseReconciliation, PurchaseReconItem, PurchaseReconciliationQuery};
use crate::purchase::enums::PurchaseReconStatus;
use crate::shared::types::pagination::PageParams;

pub struct PurchaseReconciliationRepo;

impl PurchaseReconciliationRepo {
    /// INSERT 对账单主表，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        supplier_id: i64,
        period: &str,
        total_amount: Decimal,
        doc_number: &str,
        remark: &str,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO purchase_reconciliations
                (doc_number, supplier_id, period, status, total_amount,
                 confirmed_amount, difference, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(doc_number)
        .bind(supplier_id)
        .bind(period)
        .bind(PurchaseReconStatus::Draft)
        .bind(total_amount)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(row.try_get("id")?)
    }

    /// 按主键查询（软删除行过滤）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<PurchaseReconciliation>> {
        sqlx::query_as::<_, PurchaseReconciliation>(
            r#"
            SELECT id, doc_number, supplier_id, period, status, total_amount,
                   confirmed_amount, difference, remark, operator_id,
                   created_at, updated_at, deleted_at
            FROM purchase_reconciliations
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await.map_err(Into::into)
    }

    /// 动态条件分页查询
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &PurchaseReconciliationQuery,
        page: &PageParams,
    ) -> Result<(Vec<PurchaseReconciliation>, u64)> {
        let where_clause = "
            WHERE deleted_at IS NULL
              AND ($1::bigint IS NULL OR supplier_id = $1)
              AND ($2::text IS NULL OR period = $2)
              AND ($3::smallint IS NULL OR status = $3)
        ";

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM purchase_reconciliations {where_clause}");
        let count_row = sqlx::query(sqlx::AssertSqlSafe(count_sql))
            .bind(q.supplier_id)
            .bind(&q.period)
            .bind(q.status)
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT id, doc_number, supplier_id, period, status, total_amount,
                    confirmed_amount, difference, remark, operator_id,
                    created_at, updated_at, deleted_at
             FROM purchase_reconciliations {where_clause}
             ORDER BY created_at DESC
             LIMIT $4 OFFSET $5"
        );
        let rows = sqlx::query_as::<_, PurchaseReconciliation>(sqlx::AssertSqlSafe(data_sql))
            .bind(q.supplier_id)
            .bind(&q.period)
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
        status: PurchaseReconStatus,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE purchase_reconciliations
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

    /// 更新确认金额和差异（乐观锁）
    pub async fn update_confirmed_amount(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        confirmed_amount: Decimal,
        difference: Decimal,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE purchase_reconciliations
            SET confirmed_amount = $1, difference = $2, updated_at = NOW()
            WHERE id = $3 AND updated_at = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(confirmed_amount)
        .bind(difference)
        .bind(id)
        .bind(updated_at)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }
}

// ---------------------------------------------------------------------------
// PurchaseReconItemRepo
// ---------------------------------------------------------------------------

pub struct PurchaseReconItemRepo;

impl PurchaseReconItemRepo {
    /// 批量 INSERT 对账明细
    pub async fn insert_items(
        executor: &mut sqlx::postgres::PgConnection,
        reconciliation_id: i64,
        items: &[NewReconItem],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO purchase_recon_items
                    (reconciliation_id, order_id, order_item_id, received_qty,
                     returned_qty, returned_amount, unit_price, amount, confirmed)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(reconciliation_id)
            .bind(item.order_id)
            .bind(item.order_item_id)
            .bind(item.received_qty)
            .bind(item.returned_qty)
            .bind(item.returned_amount)
            .bind(item.unit_price)
            .bind(item.amount)
            .bind(false)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按对账单主表 id 查询全部明细
    pub async fn list_by_reconciliation_id(
        executor: &mut sqlx::postgres::PgConnection,
        reconciliation_id: i64,
    ) -> Result<Vec<PurchaseReconItem>> {
        sqlx::query_as::<_, PurchaseReconItem>(
            r#"
            SELECT id, reconciliation_id, order_id, order_item_id, received_qty,
                   returned_qty, returned_amount, unit_price, amount, confirmed
            FROM purchase_recon_items
            WHERE reconciliation_id = $1
            "#,
        )
        .bind(reconciliation_id)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }

    /// 确认单条对账明细（confirmed = true）
    pub async fn confirm_item(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE purchase_recon_items
            SET confirmed = true
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .execute(executor)
        .await?;
        Ok(())
    }
}

/// 对账明细插入参数（由 service 层构建）
pub struct NewReconItem {
    pub order_id: i64,
    pub order_item_id: i64,
    pub received_qty: Decimal,
    pub returned_qty: Decimal,
    pub returned_amount: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}
