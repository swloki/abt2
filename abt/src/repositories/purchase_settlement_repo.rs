//! 采购结算数据访问层
//!
//! 提供采购对账单、发票、付款的数据库 CRUD 操作。

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};

use crate::models::{
    InvoiceDetail, InvoiceQuery, PaymentDetail, PaymentQuery, PurchaseInvoice, PurchasePayment,
    PurchaseStatement, StatementDetail, StatementItem, StatementQuery,
};
use crate::repositories::Executor;

// ============================================================================
// StatementRepo
// ============================================================================

/// 采购对账单数据仓库
pub struct StatementRepo;

impl StatementRepo {
    /// 创建对账单，返回 statement_id
    pub async fn insert(
        executor: Executor<'_>,
        statement_no: &str,
        supplier_id: i64,
        period_start: NaiveDate,
        period_end: NaiveDate,
        total_amount: Decimal,
        operator_id: Option<i64>,
    ) -> Result<i64> {
        let statement_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO purchase_statements
                (statement_no, supplier_id, period_start, period_end, total_amount, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING statement_id
            "#,
        )
        .bind(statement_no)
        .bind(supplier_id)
        .bind(period_start)
        .bind(period_end)
        .bind(total_amount)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(statement_id)
    }

    /// 根据 ID 查找对账单
    pub async fn find_by_id(
        pool: &PgPool,
        statement_id: i64,
    ) -> Result<Option<PurchaseStatement>> {
        let row = sqlx::query_as::<_, PurchaseStatement>(
            "SELECT statement_id, statement_no, supplier_id, period_start, period_end, \
             total_amount, status, remark, operator_id, created_at, updated_at \
             FROM purchase_statements WHERE statement_id = $1",
        )
        .bind(statement_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 分页查询对账单列表（含供应商名称）
    pub async fn query(pool: &PgPool, query: &StatementQuery) -> Result<Vec<StatementDetail>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT ps.statement_id, ps.statement_no, ps.supplier_id, s.supplier_name, \
             ps.period_start, ps.period_end, ps.total_amount, ps.status, ps.remark, \
             ps.operator_id, ps.created_at, ps.updated_at \
             FROM purchase_statements ps \
             LEFT JOIN suppliers s ON ps.supplier_id = s.supplier_id \
             WHERE 1=1",
        );

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND ps.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(status) = query.status {
            qb.push(" AND ps.status = ");
            qb.push_bind(status);
        }

        if let Some(period_start) = query.period_start {
            qb.push(" AND ps.period_start >= ");
            qb.push_bind(period_start);
        }

        if let Some(period_end) = query.period_end {
            qb.push(" AND ps.period_end <= ");
            qb.push_bind(period_end);
        }

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        qb.push(" ORDER BY ps.statement_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        // Manually map rows since StatementDetail has JOIN fields
        let rows = qb.build().fetch_all(pool).await?;

        let items = rows
            .iter()
            .map(|row| StatementDetail {
                statement_id: row.get("statement_id"),
                statement_no: row.get("statement_no"),
                supplier_id: row.get("supplier_id"),
                supplier_name: row.get("supplier_name"),
                period_start: row.get("period_start"),
                period_end: row.get("period_end"),
                total_amount: row.get("total_amount"),
                status: row.get("status"),
                remark: row.get("remark"),
                operator_id: row.get("operator_id"),
                created_at: row.get::<DateTime<Utc>, _>("created_at"),
                updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
            })
            .collect();

        Ok(items)
    }

    /// 查询对账单总数
    pub async fn query_count(pool: &PgPool, query: &StatementQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM purchase_statements ps WHERE 1=1",
        );

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND ps.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(status) = query.status {
            qb.push(" AND ps.status = ");
            qb.push_bind(status);
        }

        if let Some(period_start) = query.period_start {
            qb.push(" AND ps.period_start >= ");
            qb.push_bind(period_start);
        }

        if let Some(period_end) = query.period_end {
            qb.push(" AND ps.period_end <= ");
            qb.push_bind(period_end);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新对账单状态
    pub async fn update_status(
        executor: Executor<'_>,
        statement_id: i64,
        status: i16,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_statements SET status = $1, updated_at = NOW() WHERE statement_id = $2",
        )
        .bind(status)
        .bind(statement_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 获取对账单当前状态
    pub async fn find_status(pool: &PgPool, statement_id: i64) -> Result<Option<i16>> {
        let status: Option<i16> = sqlx::query_scalar(
            "SELECT status FROM purchase_statements WHERE statement_id = $1",
        )
        .bind(statement_id)
        .fetch_optional(pool)
        .await?;

        Ok(status)
    }

    /// 批量插入对账单行项目
    pub async fn insert_items(executor: Executor<'_>, items: &[StatementItem]) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO purchase_statement_items
                    (statement_id, po_id, po_no, product_id, product_name, quantity, unit_price, amount)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(item.statement_id)
            .bind(item.po_id)
            .bind(&item.po_no)
            .bind(item.product_id)
            .bind(&item.product_name)
            .bind(item.quantity)
            .bind(item.unit_price)
            .bind(item.amount)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 查询对账单下的所有行项目
    pub async fn find_items(pool: &PgPool, statement_id: i64) -> Result<Vec<StatementItem>> {
        let rows = sqlx::query_as::<_, StatementItem>(
            "SELECT item_id, statement_id, po_id, po_no, product_id, product_name, \
             quantity, unit_price, amount \
             FROM purchase_statement_items WHERE statement_id = $1 ORDER BY item_id",
        )
        .bind(statement_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}

// ============================================================================
// InvoiceRepo
// ============================================================================

/// 采购发票数据仓库
pub struct InvoiceRepo;

impl InvoiceRepo {
    /// 创建发票，返回 invoice_id
    pub async fn insert(
        executor: Executor<'_>,
        invoice_no: &str,
        supplier_id: i64,
        statement_id: Option<i64>,
        invoice_amount: Decimal,
        invoice_date: NaiveDate,
        remark: Option<&str>,
        operator_id: Option<i64>,
    ) -> Result<i64> {
        let invoice_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO purchase_invoices
                (invoice_no, supplier_id, statement_id, invoice_amount, invoice_date, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING invoice_id
            "#,
        )
        .bind(invoice_no)
        .bind(supplier_id)
        .bind(statement_id)
        .bind(invoice_amount)
        .bind(invoice_date)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(invoice_id)
    }

    /// 根据 ID 查找发票
    pub async fn find_by_id(pool: &PgPool, invoice_id: i64) -> Result<Option<PurchaseInvoice>> {
        let row = sqlx::query_as::<_, PurchaseInvoice>(
            "SELECT invoice_id, invoice_no, supplier_id, statement_id, invoice_amount, \
             invoice_date, status, remark, operator_id, created_at \
             FROM purchase_invoices WHERE invoice_id = $1",
        )
        .bind(invoice_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 分页查询发票列表（含供应商名称和对账单编号）
    pub async fn query(pool: &PgPool, query: &InvoiceQuery) -> Result<Vec<InvoiceDetail>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT pi.invoice_id, pi.invoice_no, pi.supplier_id, s.supplier_name, \
             pi.statement_id, ps.statement_no, pi.invoice_amount, pi.invoice_date, \
             pi.status, pi.remark, pi.operator_id, pi.created_at \
             FROM purchase_invoices pi \
             LEFT JOIN suppliers s ON pi.supplier_id = s.supplier_id \
             LEFT JOIN purchase_statements ps ON pi.statement_id = ps.statement_id \
             WHERE 1=1",
        );

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND pi.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(statement_id) = query.statement_id {
            qb.push(" AND pi.statement_id = ");
            qb.push_bind(statement_id);
        }

        if let Some(status) = query.status {
            qb.push(" AND pi.status = ");
            qb.push_bind(status);
        }

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        qb.push(" ORDER BY pi.invoice_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let rows = qb.build().fetch_all(pool).await?;

        let items = rows
            .iter()
            .map(|row| InvoiceDetail {
                invoice_id: row.get("invoice_id"),
                invoice_no: row.get("invoice_no"),
                supplier_id: row.get("supplier_id"),
                supplier_name: row.get("supplier_name"),
                statement_id: row.get("statement_id"),
                statement_no: row.get("statement_no"),
                invoice_amount: row.get("invoice_amount"),
                invoice_date: row.get("invoice_date"),
                status: row.get("status"),
                remark: row.get("remark"),
                operator_id: row.get("operator_id"),
                created_at: row.get::<DateTime<Utc>, _>("created_at"),
            })
            .collect();

        Ok(items)
    }

    /// 查询发票总数
    pub async fn query_count(pool: &PgPool, query: &InvoiceQuery) -> Result<i64> {
        let mut qb =
            sqlx::QueryBuilder::new("SELECT count(*) FROM purchase_invoices pi WHERE 1=1");

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND pi.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(statement_id) = query.statement_id {
            qb.push(" AND pi.statement_id = ");
            qb.push_bind(statement_id);
        }

        if let Some(status) = query.status {
            qb.push(" AND pi.status = ");
            qb.push_bind(status);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新发票状态
    pub async fn update_status(executor: Executor<'_>, invoice_id: i64, status: i16) -> Result<()> {
        sqlx::query("UPDATE purchase_invoices SET status = $1 WHERE invoice_id = $2")
            .bind(status)
            .bind(invoice_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 获取发票当前状态
    pub async fn find_status(pool: &PgPool, invoice_id: i64) -> Result<Option<i16>> {
        let status: Option<i16> = sqlx::query_scalar(
            "SELECT status FROM purchase_invoices WHERE invoice_id = $1",
        )
        .bind(invoice_id)
        .fetch_optional(pool)
        .await?;

        Ok(status)
    }
}

// ============================================================================
// PaymentRepo
// ============================================================================

/// 采购付款数据仓库
pub struct PaymentRepo;

impl PaymentRepo {
    /// 创建付款，返回 payment_id
    pub async fn insert(
        executor: Executor<'_>,
        payment_no: &str,
        supplier_id: i64,
        invoice_id: Option<i64>,
        payment_amount: Decimal,
        payment_method: Option<&str>,
        remark: Option<&str>,
        operator_id: Option<i64>,
    ) -> Result<i64> {
        let payment_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO purchase_payments
                (payment_no, supplier_id, invoice_id, payment_amount, payment_method, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING payment_id
            "#,
        )
        .bind(payment_no)
        .bind(supplier_id)
        .bind(invoice_id)
        .bind(payment_amount)
        .bind(payment_method)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(payment_id)
    }

    /// 根据 ID 查找付款
    pub async fn find_by_id(pool: &PgPool, payment_id: i64) -> Result<Option<PurchasePayment>> {
        let row = sqlx::query_as::<_, PurchasePayment>(
            "SELECT payment_id, payment_no, supplier_id, invoice_id, payment_amount, \
             payment_method, status, remark, operator_id, created_at, updated_at \
             FROM purchase_payments WHERE payment_id = $1",
        )
        .bind(payment_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 分页查询付款列表（含供应商名称和发票编号）
    pub async fn query(pool: &PgPool, query: &PaymentQuery) -> Result<Vec<PaymentDetail>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT pp.payment_id, pp.payment_no, pp.supplier_id, s.supplier_name, \
             pp.invoice_id, pi.invoice_no, pp.payment_amount, pp.payment_method, \
             pp.status, pp.remark, pp.operator_id, pp.created_at, pp.updated_at \
             FROM purchase_payments pp \
             LEFT JOIN suppliers s ON pp.supplier_id = s.supplier_id \
             LEFT JOIN purchase_invoices pi ON pp.invoice_id = pi.invoice_id \
             WHERE 1=1",
        );

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND pp.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(status) = query.status {
            qb.push(" AND pp.status = ");
            qb.push_bind(status);
        }

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        qb.push(" ORDER BY pp.payment_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let rows = qb.build().fetch_all(pool).await?;

        let items = rows
            .iter()
            .map(|row| PaymentDetail {
                payment_id: row.get("payment_id"),
                payment_no: row.get("payment_no"),
                supplier_id: row.get("supplier_id"),
                supplier_name: row.get("supplier_name"),
                invoice_id: row.get("invoice_id"),
                invoice_no: row.get("invoice_no"),
                payment_amount: row.get("payment_amount"),
                payment_method: row.get("payment_method"),
                status: row.get("status"),
                remark: row.get("remark"),
                operator_id: row.get("operator_id"),
                created_at: row.get::<DateTime<Utc>, _>("created_at"),
                updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
            })
            .collect();

        Ok(items)
    }

    /// 查询付款总数
    pub async fn query_count(pool: &PgPool, query: &PaymentQuery) -> Result<i64> {
        let mut qb =
            sqlx::QueryBuilder::new("SELECT count(*) FROM purchase_payments pp WHERE 1=1");

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND pp.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(status) = query.status {
            qb.push(" AND pp.status = ");
            qb.push_bind(status);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新付款状态
    pub async fn update_status(executor: Executor<'_>, payment_id: i64, status: i16) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_payments SET status = $1, updated_at = NOW() WHERE payment_id = $2",
        )
        .bind(status)
        .bind(payment_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 获取付款当前状态
    pub async fn find_status(pool: &PgPool, payment_id: i64) -> Result<Option<i16>> {
        let status: Option<i16> = sqlx::query_scalar(
            "SELECT status FROM purchase_payments WHERE payment_id = $1",
        )
        .bind(payment_id)
        .fetch_optional(pool)
        .await?;

        Ok(status)
    }
}
