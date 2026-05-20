//! 销售订单数据访问层
//!
//! 提供销售订单主表及行项目的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{SalesOrder, SalesOrderItem, SalesOrderQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

/// 销售订单数据仓库
pub struct SalesOrderRepo;

impl SalesOrderRepo {
    // === 主表 ===

    /// 创建销售订单，返回 order_id
    pub async fn insert(executor: Executor<'_>, order: &SalesOrder) -> Result<i64> {
        let order_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO sales_orders (order_no, quotation_id, customer_name, contact_person, contact_phone, status, total_amount, remark, delivery_date, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING order_id
            "#,
        )
        .bind(&order.order_no)
        .bind(order.quotation_id)
        .bind(&order.customer_name)
        .bind(&order.contact_person)
        .bind(&order.contact_phone)
        .bind(order.status)
        .bind(order.total_amount)
        .bind(&order.remark)
        .bind(order.delivery_date)
        .bind(order.operator_id)
        .fetch_one(executor)
        .await?;

        Ok(order_id)
    }

    /// 更新销售订单（不含 order_no、status 和行项目）
    pub async fn update(executor: Executor<'_>, order: &SalesOrder) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sales_orders
            SET customer_name = $1, contact_person = $2, contact_phone = $3, remark = $4, delivery_date = $5, total_amount = $6, updated_at = NOW()
            WHERE order_id = $7 AND deleted_at IS NULL
            "#,
        )
        .bind(&order.customer_name)
        .bind(&order.contact_person)
        .bind(&order.contact_phone)
        .bind(&order.remark)
        .bind(order.delivery_date)
        .bind(order.total_amount)
        .bind(order.order_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 仅更新销售订单头部信息
    pub async fn update_header(
        pool: &PgPool,
        order_id: i64,
        customer_name: String,
        contact_person: Option<String>,
        contact_phone: Option<String>,
        remark: Option<String>,
        delivery_date: Option<chrono::NaiveDateTime>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sales_orders
            SET customer_name = $1, contact_person = $2, contact_phone = $3, remark = $4, delivery_date = $5, updated_at = NOW()
            WHERE order_id = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(customer_name)
        .bind(contact_person)
        .bind(contact_phone)
        .bind(remark)
        .bind(delivery_date)
        .bind(order_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 软删除销售订单
    pub async fn soft_delete(executor: Executor<'_>, order_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE sales_orders SET deleted_at = NOW() WHERE order_id = $1 AND deleted_at IS NULL",
        )
        .bind(order_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 根据 ID 查找销售订单
    pub async fn find_by_id(pool: &PgPool, order_id: i64) -> Result<Option<SalesOrder>> {
        let row = sqlx::query_as::<_, SalesOrder>(
            "SELECT order_id, order_no, quotation_id, customer_name, contact_person, contact_phone, \
             status, total_amount, remark, delivery_date, operator_id, created_at, updated_at, deleted_at \
             FROM sales_orders WHERE order_id = $1 AND deleted_at IS NULL",
        )
        .bind(order_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 查询销售订单列表
    pub async fn query(pool: &PgPool, q: &SalesOrderQuery) -> Result<Vec<SalesOrder>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT order_id, order_no, quotation_id, customer_name, contact_person, contact_phone, \
             status, total_amount, remark, delivery_date, operator_id, created_at, updated_at, deleted_at \
             FROM sales_orders WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (order_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(12).clamp(1, 100);

        qb.push(" ORDER BY order_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb.build_query_as::<SalesOrder>().fetch_all(pool).await?;
        Ok(result)
    }

    /// 查询销售订单总数
    pub async fn query_count(pool: &PgPool, q: &SalesOrderQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM sales_orders WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (order_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新销售订单状态
    pub async fn update_status(executor: Executor<'_>, order_id: i64, status: i16) -> Result<()> {
        sqlx::query(
            "UPDATE sales_orders SET status = $1, updated_at = NOW() WHERE order_id = $2 AND deleted_at IS NULL",
        )
        .bind(status)
        .bind(order_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    // === 行项目 ===

    /// 批量插入销售订单行项目
    pub async fn insert_items(executor: Executor<'_>, items: &[SalesOrderItem]) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO sales_order_items (order_id, product_id, product_code, product_name, unit, unit_price, quantity, discount, subtotal, shipped_qty, returned_qty, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                "#,
            )
            .bind(item.order_id)
            .bind(item.product_id)
            .bind(&item.product_code)
            .bind(&item.product_name)
            .bind(&item.unit)
            .bind(item.unit_price)
            .bind(item.quantity)
            .bind(item.discount)
            .bind(item.subtotal)
            .bind(item.shipped_qty)
            .bind(item.returned_qty)
            .bind(&item.remark)
            .execute(&mut *executor)
            .await?;
        }

        Ok(())
    }

    /// 根据销售订单 ID 查询行项目
    pub async fn find_by_order_id(pool: &PgPool, order_id: i64) -> Result<Vec<SalesOrderItem>> {
        let rows = sqlx::query_as::<_, SalesOrderItem>(
            "SELECT item_id, order_id, product_id, product_code, product_name, unit, \
             unit_price, quantity, discount, subtotal, shipped_qty, returned_qty, remark, created_at \
             FROM sales_order_items WHERE order_id = $1 ORDER BY item_id",
        )
        .bind(order_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 增加已发货数量
    pub async fn update_shipped_qty(executor: Executor<'_>, item_id: i64, qty: rust_decimal::Decimal) -> Result<()> {
        sqlx::query(
            "UPDATE sales_order_items SET shipped_qty = shipped_qty + $1 WHERE item_id = $2",
        )
        .bind(qty)
        .bind(item_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 增加已退货数量
    pub async fn update_returned_qty(executor: Executor<'_>, item_id: i64, qty: rust_decimal::Decimal) -> Result<()> {
        sqlx::query(
            "UPDATE sales_order_items SET returned_qty = returned_qty + $1 WHERE item_id = $2",
        )
        .bind(qty)
        .bind(item_id)
        .execute(executor)
        .await?;

        Ok(())
    }
}
