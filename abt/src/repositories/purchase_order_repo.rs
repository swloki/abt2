use anyhow::Result;
use sqlx::PgPool;

use crate::models::{PurchaseOrder, PurchaseOrderDetail, PurchaseOrderItem, PurchaseOrderQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

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

    /// 批量更新采购订单状态
    pub async fn batch_update_status(executor: Executor<'_>, po_ids: &[i64], status: i16) -> Result<()> {
        if po_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "UPDATE purchase_orders SET status = $1, updated_at = NOW() WHERE po_id = ANY($2)",
        )
        .bind(status)
        .bind(po_ids)
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

pub struct PurchaseOrderItemRepo;

impl PurchaseOrderItemRepo {
    pub async fn insert_batch(executor: Executor<'_>, items: &[PurchaseOrderItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let po_ids: Vec<i64> = items.iter().map(|i| i.po_id).collect();
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        let product_codes: Vec<Option<&str>> = items.iter().map(|i| i.product_code.as_deref()).collect();
        let product_names: Vec<Option<&str>> = items.iter().map(|i| i.product_name.as_deref()).collect();
        let units: Vec<Option<&str>> = items.iter().map(|i| i.unit.as_deref()).collect();
        let unit_prices: Vec<rust_decimal::Decimal> = items.iter().map(|i| i.unit_price).collect();
        let quantities: Vec<rust_decimal::Decimal> = items.iter().map(|i| i.quantity).collect();
        let received_qtys: Vec<rust_decimal::Decimal> = items.iter().map(|i| i.received_qty).collect();
        let subtotals: Vec<rust_decimal::Decimal> = items.iter().map(|i| i.subtotal).collect();
        let remarks: Vec<Option<&str>> = items.iter().map(|i| i.remark.as_deref()).collect();

        sqlx::query(
            r#"
            INSERT INTO purchase_order_items
                (po_id, product_id, product_code, product_name, unit, unit_price, quantity, received_qty, subtotal, remark)
            SELECT * FROM UNNEST(
                $1::bigint[], $2::bigint[], $3::varchar[], $4::varchar[], $5::varchar[],
                $6::decimal[], $7::decimal[], $8::decimal[], $9::decimal[], $10::varchar[]
            )
            "#,
        )
        .bind(&po_ids)
        .bind(&product_ids)
        .bind(&product_codes)
        .bind(&product_names)
        .bind(&units)
        .bind(&unit_prices)
        .bind(&quantities)
        .bind(&received_qtys)
        .bind(&subtotals)
        .bind(&remarks)
        .execute(executor)
        .await?;
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
