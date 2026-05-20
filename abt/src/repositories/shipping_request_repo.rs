//! 发货申请数据访问层
//!
//! 提供发货申请主表及行项目的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{ShippingRequest, ShippingRequestItem, ShippingRequestQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

/// 发货申请数据仓库
pub struct ShippingRequestRepo;

impl ShippingRequestRepo {
    // === 主表 ===

    /// 创建发货申请，返回 request_id
    pub async fn insert(executor: Executor<'_>, request: &ShippingRequest) -> Result<i64> {
        let request_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO shipping_requests (request_no, order_id, customer_name, status, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING request_id
            "#,
        )
        .bind(&request.request_no)
        .bind(request.order_id)
        .bind(&request.customer_name)
        .bind(request.status)
        .bind(&request.remark)
        .bind(request.operator_id)
        .fetch_one(executor)
        .await?;

        Ok(request_id)
    }

    /// 更新发货申请（remark、customer_name）
    pub async fn update(executor: Executor<'_>, request: &ShippingRequest) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE shipping_requests
            SET customer_name = $1, remark = $2, updated_at = NOW()
            WHERE request_id = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(&request.customer_name)
        .bind(&request.remark)
        .bind(request.request_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 软删除发货申请
    pub async fn soft_delete(executor: Executor<'_>, request_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE shipping_requests SET deleted_at = NOW() WHERE request_id = $1 AND deleted_at IS NULL",
        )
        .bind(request_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 根据 ID 查找发货申请
    pub async fn find_by_id(pool: &PgPool, request_id: i64) -> Result<Option<ShippingRequest>> {
        let row = sqlx::query_as::<_, ShippingRequest>(
            "SELECT request_id, request_no, order_id, customer_name, status, remark, operator_id, \
             confirmed_at, shipped_at, created_at, updated_at, deleted_at \
             FROM shipping_requests WHERE request_id = $1 AND deleted_at IS NULL",
        )
        .bind(request_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 查询发货申请列表
    pub async fn query(pool: &PgPool, q: &ShippingRequestQuery) -> Result<Vec<ShippingRequest>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT request_id, request_no, order_id, customer_name, status, remark, operator_id, \
             confirmed_at, shipped_at, created_at, updated_at, deleted_at \
             FROM shipping_requests WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (request_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(order_id) = q.order_id {
            qb.push(" AND order_id = ");
            qb.push_bind(order_id);
        }

        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(12).clamp(1, 100);

        qb.push(" ORDER BY request_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb.build_query_as::<ShippingRequest>().fetch_all(pool).await?;
        Ok(result)
    }

    /// 查询发货申请总数
    pub async fn query_count(pool: &PgPool, q: &ShippingRequestQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM shipping_requests WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (request_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(order_id) = q.order_id {
            qb.push(" AND order_id = ");
            qb.push_bind(order_id);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新发货申请状态
    pub async fn update_status(executor: Executor<'_>, request_id: i64, status: i16) -> Result<()> {
        sqlx::query(
            "UPDATE shipping_requests SET status = $1, updated_at = NOW() WHERE request_id = $2 AND deleted_at IS NULL",
        )
        .bind(status)
        .bind(request_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 设置确认时间
    pub async fn update_confirmed_at(executor: Executor<'_>, request_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE shipping_requests SET confirmed_at = NOW(), updated_at = NOW() WHERE request_id = $1 AND deleted_at IS NULL",
        )
        .bind(request_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 设置发货时间
    pub async fn update_shipped_at(executor: Executor<'_>, request_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE shipping_requests SET shipped_at = NOW(), updated_at = NOW() WHERE request_id = $1 AND deleted_at IS NULL",
        )
        .bind(request_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    // === 行项目 ===

    /// 批量插入发货申请行项目
    pub async fn insert_items(executor: Executor<'_>, items: &[ShippingRequestItem]) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO shipping_request_items (request_id, order_item_id, product_id, product_code, product_name, unit, quantity, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(item.request_id)
            .bind(item.order_item_id)
            .bind(item.product_id)
            .bind(&item.product_code)
            .bind(&item.product_name)
            .bind(&item.unit)
            .bind(item.quantity)
            .bind(&item.remark)
            .execute(&mut *executor)
            .await?;
        }

        Ok(())
    }

    /// 删除指定发货申请的所有行项目
    pub async fn delete_by_request(executor: Executor<'_>, request_id: i64) -> Result<()> {
        sqlx::query(
            "DELETE FROM shipping_request_items WHERE request_id = $1",
        )
        .bind(request_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 根据发货申请 ID 查询行项目
    pub async fn find_by_request_id(pool: &PgPool, request_id: i64) -> Result<Vec<ShippingRequestItem>> {
        let rows = sqlx::query_as::<_, ShippingRequestItem>(
            "SELECT item_id, request_id, order_item_id, product_id, product_code, product_name, unit, \
             quantity, remark, created_at \
             FROM shipping_request_items WHERE request_id = $1 ORDER BY item_id",
        )
        .bind(request_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
