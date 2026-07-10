use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use std::collections::HashMap;

use crate::shared::types::Result;

use super::model::{
    CreateOrderItemRequest, CreatePurchaseOrderRequest, PurchaseOrder, PurchaseOrderItem,
    PurchaseOrderQuery, line_amounts,
};
use crate::purchase::enums::PurchaseOrderStatus;
use crate::purchase::tax::repo::TaxRateRepo;
use crate::shared::types::pagination::{DataScope, PageParams};

/// 批量加载明细涉及的税率映射（`tax_rate_id -> rate`），供金额计算复用，避免逐行查询。
pub async fn load_tax_rate_map(
    executor: &mut sqlx::postgres::PgConnection,
    items: &[CreateOrderItemRequest],
) -> Result<HashMap<i64, Decimal>> {
    let tax_rate_ids: Vec<i64> = items
        .iter()
        .filter_map(|i| i.tax_rate_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let tax_rates = if tax_rate_ids.is_empty() {
        Vec::new()
    } else {
        TaxRateRepo::get_by_ids(executor, &tax_rate_ids).await?
    };
    Ok(tax_rates.into_iter().map(|t| (t.id, t.rate)).collect())
}

pub struct PurchaseOrderRepo;

impl PurchaseOrderRepo {
    /// INSERT 采购订单主表，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreatePurchaseOrderRequest,
        doc_number: &str,
        total_amount: Decimal,
        amount_untaxed: Decimal,
        amount_tax: Decimal,
        amount_total: Decimal,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO purchase_orders
                (doc_number, supplier_id, order_date, expected_delivery_date, status,
                 total_amount, payment_terms, delivery_address, remark, operator_id,
                 currency_code, currency_rate, amount_untaxed, amount_tax,
                 amount_total, discount_amount)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15, $16)
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
        .bind(&req.currency_code)
        .bind(req.currency_rate)
        .bind(amount_untaxed)
        .bind(amount_tax)
        .bind(amount_total)
        .bind(req.discount_amount)
        .fetch_one(executor)
        .await?;

        Ok(row.try_get("id")?)
    }

    /// 按主键查询（软删除行过滤）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<PurchaseOrder>> {
        sqlx::query_as::<_, PurchaseOrder>(
            r#"
            SELECT id, doc_number, supplier_id, order_date, expected_delivery_date,
                   status, total_amount, currency_code, currency_rate,
                   amount_untaxed, amount_tax, amount_total, discount_amount,
                   payment_terms, delivery_address, remark,
                   payment_schedule_generated,
                   invoice_status, per_billed,
                   operator_id, created_at, updated_at, deleted_at
            FROM purchase_orders
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await.map_err(Into::into)
    }

    /// 批量加载多个 PO（避免逐个 get_by_id 的 N+1）；不含已软删除/不存在的 id（调用方按 len 校验）。
    pub async fn get_by_ids(
        executor: &mut sqlx::postgres::PgConnection,
        ids: &[i64],
    ) -> Result<Vec<PurchaseOrder>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, PurchaseOrder>(
            r#"
            SELECT id, doc_number, supplier_id, order_date, expected_delivery_date,
                   status, total_amount, currency_code, currency_rate,
                   amount_untaxed, amount_tax, amount_total, discount_amount,
                   payment_terms, delivery_address, remark,
                   payment_schedule_generated,
                   invoice_status, per_billed,
                   operator_id, created_at, updated_at, deleted_at
            FROM purchase_orders
            WHERE id = ANY($1) AND deleted_at IS NULL
            "#,
        )
        .bind(ids)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }

    /// 动态条件分页查询（支持 DataScope 行级权限过滤）
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &PurchaseOrderQuery,
        page: &PageParams,
        scope: (DataScope, i64, Option<i64>),
    ) -> Result<(Vec<PurchaseOrder>, u64)> {
        let (data_scope, operator_id, _department_id) = scope;
        // 状态过滤归一化：statuses（多值，优先）或 status（单值）→ Option<Vec<i16>>，
        // 统一用 `status = ANY($2::int2[])`。单值变 1 元素数组，与原 `status = $2` 等价（向后兼容）。
        let status_vals: Option<Vec<i16>> = match (q.statuses.as_ref(), q.status) {
            (Some(ss), _) if !ss.is_empty() => Some(ss.iter().map(|s| s.as_i16()).collect()),
            (_, Some(s)) => Some(vec![s.as_i16()]),
            _ => None,
        };
        // purchase_orders 无 department_id，Department 降级为 SelfOnly
        // 占位符布局：$1 supplier_id, $2 status[](int2[]), $3 order_date_start, $4 order_date_end,
        //             $5 doc_number, $6 product_code
        // count 追加 scope $7；data 追加 LIMIT $7 OFFSET $8 + scope $9
        let count_scope = if matches!(data_scope, DataScope::All) {
            ""
        } else {
            "AND purchase_orders.operator_id = $7"
        };
        let data_scope_clause = if matches!(data_scope, DataScope::All) {
            ""
        } else {
            "AND purchase_orders.operator_id = $9"
        };
        let where_clause = format!(
            "WHERE purchase_orders.deleted_at IS NULL
              AND ($1::bigint IS NULL OR purchase_orders.supplier_id = $1)
              AND ($2::int2[] IS NULL OR purchase_orders.status = ANY($2))
              AND ($3::date IS NULL OR purchase_orders.order_date >= $3)
              AND ($4::date IS NULL OR purchase_orders.order_date <= $4)
              AND ($5::text IS NULL OR purchase_orders.doc_number ILIKE '%' || $5 || '%')
              AND ($6::text IS NULL OR EXISTS (
                    SELECT 1 FROM purchase_order_items poi
                    JOIN products p ON p.product_id = poi.product_id AND p.deleted_at IS NULL
                    WHERE poi.order_id = purchase_orders.id
                      AND (p.product_code ILIKE '%' || $6 || '%'
                           OR p.pdt_name ILIKE '%' || $6 || '%')))"
        );

        // 排序：白名单列名 + 方向（防注入）。sort=supplier 需 LEFT JOIN suppliers；
        // date=交期（UI 显示 expected_delivery_date，可 NULL）；默认按 order_date（业务日期，NOT NULL）
        let (order_col, default_asc, need_join, nullable) = match q.sort.as_deref() {
            Some("amount") => ("total_amount", false, false, false),
            Some("supplier") => ("s.supplier_name", true, true, true),
            Some("doc") => ("doc_number", false, false, false),
            Some("date") => ("expected_delivery_date", false, false, true),
            _ => ("order_date", false, false, false),
        };
        let asc = match q.dir.as_deref() {
            Some("asc") => true,
            Some("desc") => false,
            _ => default_asc,
        };
        let order_clause = format!(
            "{order_col} {}{}",
            if asc { "ASC" } else { "DESC" },
            if need_join || nullable { " NULLS LAST" } else { "" }
        );
        let join_clause = if need_join {
            "LEFT JOIN suppliers s ON s.supplier_id = purchase_orders.supplier_id AND s.deleted_at IS NULL"
        } else {
            ""
        };

        // Count
        let count_sql =
            format!("SELECT COUNT(*) AS cnt FROM purchase_orders {where_clause} {count_scope}");
        let mut count_query = sqlx::query(sqlx::AssertSqlSafe(count_sql))
            .bind(q.supplier_id)
            .bind(&status_vals)
            .bind(q.order_date_start)
            .bind(q.order_date_end)
            .bind(q.doc_number.as_deref())
            .bind(q.product_code.as_deref());
        if !matches!(data_scope, DataScope::All) {
            count_query = count_query.bind(operator_id);
        }
        let count_row = count_query.fetch_one(&mut *executor).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT purchase_orders.id, purchase_orders.doc_number, purchase_orders.supplier_id, purchase_orders.order_date, purchase_orders.expected_delivery_date,
                    purchase_orders.status, purchase_orders.total_amount, purchase_orders.currency_code, purchase_orders.currency_rate,
                    purchase_orders.amount_untaxed, purchase_orders.amount_tax, purchase_orders.amount_total, purchase_orders.discount_amount,
                    purchase_orders.payment_terms, purchase_orders.delivery_address, purchase_orders.remark,
                    purchase_orders.payment_schedule_generated,
                    purchase_orders.invoice_status, purchase_orders.per_billed,
                    purchase_orders.operator_id, purchase_orders.created_at, purchase_orders.updated_at, purchase_orders.deleted_at
             FROM purchase_orders {join_clause} {where_clause} {data_scope_clause}
             ORDER BY {order_clause}
             LIMIT $7 OFFSET $8"
        );
        let mut data_query = sqlx::query_as::<_, PurchaseOrder>(sqlx::AssertSqlSafe(data_sql))
            .bind(q.supplier_id)
            .bind(&status_vals)
            .bind(q.order_date_start)
            .bind(q.order_date_end)
            .bind(q.doc_number.as_deref())
            .bind(q.product_code.as_deref())
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
        status: PurchaseOrderStatus,
        updated_at: &DateTime<Utc>,
    ) -> Result<u64> {
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

    /// 更新订单头字段（仅草稿状态可调用）
    pub async fn update_fields(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        req: &super::model::UpdatePurchaseOrderRequest,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE purchase_orders
            SET supplier_id = $2,
                expected_delivery_date = $3,
                payment_terms = $4,
                delivery_address = $5,
                remark = $6,
                currency_code = $7,
                currency_rate = $8,
                discount_amount = $9,
                updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(req.supplier_id)
        .bind(req.expected_delivery_date)
        .bind(&req.payment_terms)
        .bind(&req.delivery_address)
        .bind(&req.remark)
        .bind(&req.currency_code)
        .bind(req.currency_rate)
        .bind(req.discount_amount)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 更新订单总金额
    pub async fn update_total_amount(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        total_amount: Decimal,
        amount_untaxed: Decimal,
        amount_tax: Decimal,
        amount_total: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_orders SET total_amount = $2, amount_untaxed = $3, amount_tax = $4, amount_total = $5, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(total_amount)
        .bind(amount_untaxed)
        .bind(amount_tax)
        .bind(amount_total)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 更新头级开票状态和百分比
    pub async fn update_invoice_status(
        executor: &mut sqlx::postgres::PgConnection,
        po_id: i64,
        status: crate::purchase::enums::InvoiceStatus,
        per_billed: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_orders SET invoice_status = $2, per_billed = $3, updated_at = NOW() WHERE id = $1",
        )
        .bind(po_id)
        .bind(status)
        .bind(per_billed)
        .execute(&mut *executor)
        .await?;
        Ok(())
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
    ) -> Result<()> {
        let tax_map = load_tax_rate_map(&mut *executor, items).await?;

        for item in items {
            let rate = item
                .tax_rate_id
                .and_then(|tid| tax_map.get(&tid).copied())
                .unwrap_or(Decimal::ZERO);
            let (amount, price_subtotal, price_tax, price_total) =
                line_amounts(item.quantity, item.unit_price, item.discount_pct, rate);
            sqlx::query(
                r#"
                INSERT INTO purchase_order_items
                    (order_id, line_no, product_id, description, quantity, unit_price,
                     amount, quotation_item_id, expected_delivery_date,
                     discount_pct, tax_rate_id, price_subtotal, price_tax, price_total)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
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
            .bind(item.discount_pct)
            .bind(item.tax_rate_id)
            .bind(price_subtotal)
            .bind(price_tax)
            .bind(price_total)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按订单主表 id 查询全部明细
    pub async fn list_by_order_id(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
    ) -> Result<Vec<PurchaseOrderItem>> {
        sqlx::query_as::<_, PurchaseOrderItem>(
            r#"
            SELECT id, order_id, line_no, product_id, description, quantity, unit_price,
                   amount, received_qty, inspected_qty, returned_qty, quotation_item_id,
                   expected_delivery_date,
                   discount_pct, tax_rate_id, price_subtotal, price_tax, price_total,
                   qty_invoiced, invoice_status
            FROM purchase_order_items
            WHERE order_id = $1
            ORDER BY line_no
            "#,
        )
        .bind(order_id)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }

    /// 批量查多个 PO 的明细（避免逐个 list_by_order_id 的 N+1）；结果含 order_id，调用方按需分组。
    pub async fn list_by_order_ids(
        executor: &mut sqlx::postgres::PgConnection,
        order_ids: &[i64],
    ) -> Result<Vec<PurchaseOrderItem>> {
        if order_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, PurchaseOrderItem>(
            r#"
            SELECT id, order_id, line_no, product_id, description, quantity, unit_price,
                   amount, received_qty, inspected_qty, returned_qty, quotation_item_id,
                   expected_delivery_date,
                   discount_pct, tax_rate_id, price_subtotal, price_tax, price_total,
                   qty_invoiced, invoice_status
            FROM purchase_order_items
            WHERE order_id = ANY($1)
            ORDER BY line_no
            "#,
        )
        .bind(order_ids)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }

    /// 按供应商查询指定期间内已收货且未关联到已确认对账单的订单明细
    pub async fn list_unreconciled_received_by_supplier(
        executor: &mut sqlx::postgres::PgConnection,
        supplier_id: i64,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> Result<Vec<PurchaseOrderItem>> {
        sqlx::query_as::<_, PurchaseOrderItem>(
            r#"
            SELECT poi.id, poi.order_id, poi.line_no, poi.product_id, poi.description,
                   poi.quantity, poi.unit_price, poi.amount, poi.received_qty,
                   poi.inspected_qty, poi.returned_qty, poi.quotation_item_id,
                   poi.expected_delivery_date,
                   poi.discount_pct, poi.tax_rate_id,
                   poi.price_subtotal, poi.price_tax, poi.price_total,
                   poi.qty_invoiced, poi.invoice_status
            FROM purchase_order_items poi
            JOIN purchase_orders po ON po.id = poi.order_id
            WHERE po.supplier_id = $1
              AND po.status IN ($2, $3, $4)
              AND po.deleted_at IS NULL
              AND poi.received_qty > 0
              AND po.order_date BETWEEN $5 AND $6
              AND NOT EXISTS (
                  SELECT 1 FROM purchase_recon_items pri
                  JOIN purchase_reconciliations pr ON pr.id = pri.reconciliation_id
                  WHERE pri.order_item_id = poi.id
                    AND pr.status >= 2
                    AND pr.deleted_at IS NULL
              )
            ORDER BY po.order_date, poi.line_no
            "#,
        )
        .bind(supplier_id)
        .bind(PurchaseOrderStatus::Confirmed)
        .bind(PurchaseOrderStatus::PartiallyReceived)
        .bind(PurchaseOrderStatus::Received)
        .bind(period_start)
        .bind(period_end)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }

    /// 删除指定订单的全部明细（编辑草稿时先删后插）
    pub async fn delete_by_order_id(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
    ) -> Result<()> {
        sqlx::query("DELETE FROM purchase_order_items WHERE order_id = $1")
            .bind(order_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }
    /// 增量累加某明细行 received_qty（PO 直接收货用；行级原子，并发部分收货串行化）。
    /// 返回更新后的 received_qty，供金额重算/状态判定读。
    pub async fn add_received_qty(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        delta: Decimal,
    ) -> Result<Decimal> {
        let updated: Decimal = sqlx::query_scalar(
            "UPDATE purchase_order_items SET received_qty = received_qty + $2 WHERE id = $1 RETURNING received_qty",
        )
        .bind(item_id)
        .bind(delta)
        .fetch_one(&mut *executor)
        .await?;
        Ok(updated)
    }

    /// 插入单行（追加模式）
    pub async fn insert_single(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
        line_no: i32,
        item: &CreateOrderItemRequest,
    ) -> Result<()> {
        let (amount, price_subtotal, _, _) =
            line_amounts(item.quantity, item.unit_price, item.discount_pct, Decimal::ZERO);
        sqlx::query(
            r#"
            INSERT INTO purchase_order_items
                (order_id, line_no, product_id, description, quantity, unit_price,
                 amount, quotation_item_id, expected_delivery_date,
                 discount_pct, tax_rate_id, price_subtotal, price_tax, price_total)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 0, $12)
            "#,
        )
        .bind(order_id)
        .bind(line_no)
        .bind(item.product_id)
        .bind(&item.description)
        .bind(item.quantity)
        .bind(item.unit_price)
        .bind(amount)
        .bind(item.quotation_item_id)
        .bind(item.expected_delivery_date)
        .bind(item.discount_pct)
        .bind(item.tax_rate_id)
        .bind(price_subtotal)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 确认后更新行字段（动态构建 SET）
    pub async fn update_fields_after_confirm(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        quantity: Option<Decimal>,
        unit_price: Option<Decimal>,
        discount_pct: Option<Decimal>,
        tax_rate_id: Option<Option<i64>>,
    ) -> Result<()> {

        // 动态构建 SET 子句（tax_rate_id 为三态：None=不改 / Some(None)=置空 / Some(Some)=设值）
        let mut sql_parts = Vec::new();
        let mut bind_idx = 2;
        if quantity.is_some() {
            sql_parts.push(format!("quantity = ${bind_idx}"));
            bind_idx += 1;
        }
        if unit_price.is_some() {
            sql_parts.push(format!("unit_price = ${bind_idx}"));
            bind_idx += 1;
        }
        if discount_pct.is_some() {
            sql_parts.push(format!("discount_pct = ${bind_idx}"));
            bind_idx += 1;
        }
        if tax_rate_id.is_some() {
            sql_parts.push(format!("tax_rate_id = ${bind_idx}"));
            bind_idx += 1;
        }

        if sql_parts.is_empty() {
            return Ok(());
        }

        let set_clause = sql_parts.join(", ");
        let sql = format!(
            "UPDATE purchase_order_items SET {set_clause}, updated_at = NOW() WHERE id = $1"
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(item_id);
        if let Some(qty) = quantity { query = query.bind(qty); }
        if let Some(price) = unit_price { query = query.bind(price); }
        if let Some(disc) = discount_pct { query = query.bind(disc); }
        if let Some(tid) = tax_rate_id { query = query.bind(tid); }
        query.execute(&mut *executor).await?;

        // Recalculate derived amounts if unit_price or discount changed
        if unit_price.is_some() || discount_pct.is_some() || quantity.is_some() {
            Self::recalc_item_amounts(executor, item_id).await?;
        }
        Ok(())
    }

    /// 重算单行金额
    async fn recalc_item_amounts(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE purchase_order_items SET
                amount = quantity * unit_price,
                price_subtotal = quantity * unit_price * (1 - discount_pct / 100),
                price_tax = quantity * unit_price * (1 - discount_pct / 100)
                    * COALESCE((SELECT rate FROM tax_rates WHERE id = purchase_order_items.tax_rate_id AND deleted_at IS NULL), 0) / 100,
                price_total = quantity * unit_price * (1 - discount_pct / 100)
                    * (1 + COALESCE((SELECT rate FROM tax_rates WHERE id = purchase_order_items.tax_rate_id AND deleted_at IS NULL), 0) / 100)
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 按主键删除行
    pub async fn delete_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
    ) -> Result<()> {
        sqlx::query("DELETE FROM purchase_order_items WHERE id = $1")
            .bind(item_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// 累加已开票数量
    pub async fn add_qty_invoiced(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        qty: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_order_items SET qty_invoiced = qty_invoiced + $2 WHERE id = $1",
        )
        .bind(item_id)
        .bind(qty)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

}
