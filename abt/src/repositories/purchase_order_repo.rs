//! 采购订单数据访问层
//!
//! 提供采购订单、行项目的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{PurchaseOrder, PurchaseOrderDetail, PurchaseOrderItem, PurchaseOrderQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

// ============================================================================
// PurchaseOrderRepo
// ============================================================================

/// 采购订单数据仓库
pub struct PurchaseOrderRepo;

impl PurchaseOrderRepo {
    /// 创建采购订单，返回 po_id
    pub async fn insert(
        executor: Executor<'_>,
        po_no: &str,
        supplier_id: i64,
        order_type: i16,
        total_amount: rust_decimal::Decimal,
        remark: Option<&str>,
        operator_id: Option<i64>,
    ) -> Result<i64> {
        let po_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO purchase_orders (po_no, supplier_id, order_type, total_amount, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING po_id
            "#,
        )
        .bind(po_no)
        .bind(supplier_id)
        .bind(order_type)
        .bind(total_amount)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(po_id)
    }

    /// 更新采购订单基本信息
    pub async fn update(
        executor: Executor<'_>,
        po_id: i64,
        supplier_id: i64,
        remark: Option<&str>,
        total_amount: rust_decimal::Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE purchase_orders
            SET supplier_id = $1, remark = $2, total_amount = $3, updated_at = NOW()
            WHERE po_id = $4
            "#,
        )
        .bind(supplier_id)
        .bind(remark)
        .bind(total_amount)
        .bind(po_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 软删除采购订单
    pub async fn soft_delete(executor: Executor<'_>, po_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_orders SET deleted_at = NOW() WHERE po_id = $1 AND deleted_at IS NULL",
        )
        .bind(po_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 根据 ID 查找采购订单（排除已删除）
    pub async fn find_by_id(pool: &PgPool, po_id: i64) -> Result<Option<PurchaseOrder>> {
        let row = sqlx::query_as::<_, PurchaseOrder>(
            "SELECT po_id, po_no, supplier_id, order_type, status, total_amount, \
             remark, operator_id, created_at, updated_at, deleted_at \
             FROM purchase_orders WHERE po_id = $1 AND deleted_at IS NULL",
        )
        .bind(po_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 分页查询采购订单列表（含供应商名称）
    pub async fn query(
        pool: &PgPool,
        query: &PurchaseOrderQuery,
    ) -> Result<Vec<PurchaseOrderDetail>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT po.po_id, po.po_no, po.supplier_id, s.supplier_name, \
             po.order_type, po.status, po.total_amount, po.remark, \
             po.operator_id, po.created_at, po.updated_at, po.deleted_at \
             FROM purchase_orders po \
             LEFT JOIN suppliers s ON po.supplier_id = s.supplier_id \
             WHERE po.deleted_at IS NULL",
        );

        if let Some(keyword) = &query.keyword
            && !keyword.is_empty()
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND po.po_no ILIKE ");
            qb.push_bind(pattern);
        }

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND po.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(order_type) = query.order_type {
            qb.push(" AND po.order_type = ");
            qb.push_bind(order_type);
        }

        if let Some(status) = query.status {
            qb.push(" AND po.status = ");
            qb.push_bind(status);
        }

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        qb.push(" ORDER BY po.po_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb
            .build_query_as::<PurchaseOrderDetail>()
            .fetch_all(pool)
            .await?;
        Ok(result)
    }

    /// 查询采购订单总数
    pub async fn query_count(pool: &PgPool, query: &PurchaseOrderQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM purchase_orders po WHERE po.deleted_at IS NULL",
        );

        if let Some(keyword) = &query.keyword
            && !keyword.is_empty()
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND po.po_no ILIKE ");
            qb.push_bind(pattern);
        }

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND po.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(order_type) = query.order_type {
            qb.push(" AND po.order_type = ");
            qb.push_bind(order_type);
        }

        if let Some(status) = query.status {
            qb.push(" AND po.status = ");
            qb.push_bind(status);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新采购订单状态
    pub async fn update_status(executor: Executor<'_>, po_id: i64, status: i16) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_orders SET status = $1, updated_at = NOW() WHERE po_id = $2",
        )
        .bind(status)
        .bind(po_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 获取采购订单当前状态（用于校验）
    pub async fn find_status(pool: &PgPool, po_id: i64) -> Result<Option<i16>> {
        let status: Option<i16> = sqlx::query_scalar(
            "SELECT status FROM purchase_orders WHERE po_id = $1 AND deleted_at IS NULL",
        )
        .bind(po_id)
        .fetch_optional(pool)
        .await?;

        Ok(status)
    }
}

// ============================================================================
// PurchaseOrderItemRepo
// ============================================================================

/// 采购订单行项目数据仓库
pub struct PurchaseOrderItemRepo;

impl PurchaseOrderItemRepo {
    /// 批量插入行项目
    pub async fn insert_batch(executor: Executor<'_>, items: &[PurchaseOrderItem]) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO purchase_order_items
                    (po_id, product_id, product_code, product_name, unit, unit_price, quantity, received_qty, subtotal, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(item.po_id)
            .bind(item.product_id)
            .bind(&item.product_code)
            .bind(&item.product_name)
            .bind(&item.unit)
            .bind(item.unit_price)
            .bind(item.quantity)
            .bind(item.received_qty)
            .bind(item.subtotal)
            .bind(&item.remark)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 删除采购订单下的所有行项目
    pub async fn delete_by_po(executor: Executor<'_>, po_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM purchase_order_items WHERE po_id = $1")
            .bind(po_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 查询采购订单下的所有行项目
    pub async fn find_by_po(pool: &PgPool, po_id: i64) -> Result<Vec<PurchaseOrderItem>> {
        let rows = sqlx::query_as::<_, PurchaseOrderItem>(
            "SELECT item_id, po_id, product_id, product_code, product_name, unit, \
             unit_price, quantity, received_qty, subtotal, remark, created_at \
             FROM purchase_order_items WHERE po_id = $1 ORDER BY item_id",
        )
        .bind(po_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
