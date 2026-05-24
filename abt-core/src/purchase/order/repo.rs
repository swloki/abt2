use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;

use super::model::{
    CreateOrderItemRequest, CreatePurchaseOrderRequest, PurchaseOrder, PurchaseOrderItem,
    PurchaseOrderQuery,
};
use crate::purchase::enums::PurchaseOrderStatus;
use crate::shared::types::pagination::{DataScope, PageParams};

pub struct PurchaseOrderRepo;

impl PurchaseOrderRepo {
    /// INSERT 采购订单主表，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreatePurchaseOrderRequest,
        doc_number: &str,
        total_amount: Decimal,
        operator_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO purchase_orders
                (doc_number, supplier_id, order_date, expected_delivery_date, status,
                 total_amount, payment_terms, delivery_address, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
        )
        .bind(doc_number)
        .bind(req.supplier_id)
        .bind(req.order_date)
        .bind(req.expected_delivery_date)
        .bind(PurchaseOrderStatus::Draft)
        .bind(total_amount)
        .bind(&req.payment_terms)
        .bind(&req.delivery_address)
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
    ) -> Result<Option<PurchaseOrder>, sqlx::Error> {
        sqlx::query_as::<_, PurchaseOrder>(
            r#"
            SELECT id, doc_number, supplier_id, order_date, expected_delivery_date,
                   status, total_amount, payment_terms, delivery_address, remark,
                   operator_id, created_at, updated_at, deleted_at
            FROM purchase_orders
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
    }

    /// 动态条件分页查询（支持 DataScope 行级权限过滤）
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &PurchaseOrderQuery,
        page: &PageParams,
        scope: (DataScope, i64, Option<i64>),
    ) -> Result<(Vec<PurchaseOrder>, u64), sqlx::Error> {
        let (data_scope, operator_id, _department_id) = scope;
        let scope_clause = match data_scope {
            DataScope::SelfOnly => "AND operator_id = $7",
            _ => "",
        };
        let where_clause = format!(
            "WHERE deleted_at IS NULL
              AND ($1::bigint IS NULL OR supplier_id = $1)
              AND ($2::smallint IS NULL OR status = $2)
              AND ($3::date IS NULL OR order_date >= $3)
              AND ($4::date IS NULL OR order_date <= $4)
              {scope_clause}"
        );

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM purchase_orders {where_clause}");
        let mut count_query = sqlx::query(&count_sql)
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.order_date_start)
            .bind(q.order_date_end);
        if matches!(data_scope, DataScope::SelfOnly) {
            count_query = count_query.bind(operator_id);
        }
        let count_row = count_query.fetch_one(&mut *executor).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT id, doc_number, supplier_id, order_date, expected_delivery_date,
                    status, total_amount, payment_terms, delivery_address, remark,
                    operator_id, created_at, updated_at, deleted_at
             FROM purchase_orders {where_clause}
             ORDER BY created_at DESC
             LIMIT $5 OFFSET $6"
        );
        let mut data_query = sqlx::query_as::<_, PurchaseOrder>(&data_sql)
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.order_date_start)
            .bind(q.order_date_end)
            .bind(limit)
            .bind(offset);
        if matches!(data_scope, DataScope::SelfOnly) {
            data_query = data_query.bind(operator_id);
        }
        let rows = data_query.fetch_all(&mut *executor).await?;

        Ok((rows, total as u64))
    }

    /// 状态变更（乐观锁：WHERE updated_at = $2）
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: PurchaseOrderStatus,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE purchase_orders
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
// PurchaseOrderItemRepo
// ---------------------------------------------------------------------------

pub struct PurchaseOrderItemRepo;

impl PurchaseOrderItemRepo {
    /// 批量 INSERT 订单明细
    pub async fn insert_items(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
        items: &[CreateOrderItemRequest],
    ) -> Result<(), sqlx::Error> {
        for item in items {
            let amount = item.quantity * item.unit_price;
            sqlx::query(
                r#"
                INSERT INTO purchase_order_items
                    (order_id, line_no, product_id, description, quantity, unit_price,
                     amount, quotation_item_id, expected_delivery_date)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(order_id)
            .bind(item.line_no)
            .bind(item.product_id)
            .bind(&item.description)
            .bind(item.quantity)
            .bind(item.unit_price)
            .bind(amount)
            .bind(item.quotation_item_id)
            .bind(item.expected_delivery_date)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按订单主表 id 查询全部明细
    pub async fn list_by_order_id(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
    ) -> Result<Vec<PurchaseOrderItem>, sqlx::Error> {
        sqlx::query_as::<_, PurchaseOrderItem>(
            r#"
            SELECT id, order_id, line_no, product_id, description, quantity, unit_price,
                   amount, received_qty, inspected_qty, returned_qty,
                   quotation_item_id, expected_delivery_date
            FROM purchase_order_items
            WHERE order_id = $1
            ORDER BY line_no
            "#,
        )
        .bind(order_id)
        .fetch_all(executor)
        .await
    }

    /// 按供应商查询所有已收货（Confirmed/PartiallyReceived/Received）订单明细
    pub async fn list_received_by_supplier(
        executor: &mut sqlx::postgres::PgConnection,
        supplier_id: i64,
    ) -> Result<Vec<PurchaseOrderItem>, sqlx::Error> {
        sqlx::query_as::<_, PurchaseOrderItem>(
            r#"
            SELECT poi.id, poi.order_id, poi.line_no, poi.product_id, poi.description,
                   poi.quantity, poi.unit_price, poi.amount, poi.received_qty,
                   poi.inspected_qty, poi.returned_qty, poi.quotation_item_id,
                   poi.expected_delivery_date
            FROM purchase_order_items poi
            JOIN purchase_orders po ON po.id = poi.order_id
            WHERE po.supplier_id = $1
              AND po.status IN (2, 3, 4)
              AND po.deleted_at IS NULL
              AND poi.received_qty > 0
            ORDER BY po.order_date, poi.line_no
            "#,
        )
        .bind(supplier_id)
        .fetch_all(executor)
        .await
    }
}
