use rust_decimal::Decimal;
use sqlx::{FromRow, Row};
use crate::shared::types::Result;

use super::model::{ProductWithoutPriceRow, StockExportRow, StockFilter, StockLedger, UpsertStockReq};
use crate::shared::types::pagination::PaginatedResult;

pub struct StockLedgerRepo;

impl StockLedgerRepo {
    /// INSERT ON CONFLICT UPDATE — 并发安全的库存累加/扣减
    pub async fn upsert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &UpsertStockReq,
    ) -> Result<StockLedger> {
        let batch = req.batch_no.as_deref().unwrap_or("");
        let row = sqlx::query(
            r#"
            INSERT INTO stock_ledger
                (product_id, warehouse_id, zone_id, bin_id, batch_no, quantity, available_qty, unit_cost, updated_at)
            VALUES ($1, $2, $3, $4, NULLIF($5, ''), $6, $6, $7, NOW())
            ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, ''))
            DO UPDATE SET
                quantity = stock_ledger.quantity + $6,
                available_qty = stock_ledger.available_qty + $6,
                unit_cost = COALESCE($7, stock_ledger.unit_cost),
                updated_at = NOW()
            RETURNING id, product_id, warehouse_id, zone_id, bin_id, batch_no,
                      quantity, reserved_qty, available_qty, unit_cost,
                      received_date, expiry_date, updated_at
            "#,
        )
        .bind(req.product_id)
        .bind(req.warehouse_id)
        .bind(req.zone_id)
        .bind(req.bin_id)
        .bind(batch)
        .bind(req.qty_delta)
        .bind(req.unit_cost)
        .fetch_one(executor)
        .await?;

        Ok(StockLedger::from_row(&row)?)
    }

    pub async fn find_by_location(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: i64,
        zone_id: i64,
        bin_id: i64,
        batch_no: Option<&str>,
    ) -> Result<Option<StockLedger>> {
        let batch = batch_no.unwrap_or("");
        let row = sqlx::query(
            r#"
            SELECT id, product_id, warehouse_id, zone_id, bin_id, batch_no,
                   quantity, reserved_qty, available_qty, unit_cost,
                   received_date, expiry_date, updated_at
            FROM stock_ledger
            WHERE product_id = $1 AND warehouse_id = $2 AND zone_id = $3
              AND bin_id = $4 AND COALESCE(batch_no, '') = $5
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(zone_id)
        .bind(bin_id)
        .bind(batch)
        .fetch_optional(executor)
        .await?;

        row.map(|r| StockLedger::from_row(&r).map_err(Into::into)).transpose()

    }

    /// 计算可用量 = SUM(quantity - reserved_qty)
    /// 设计要求：不用反范式 available_qty 字段，由 quantity - reserved_qty 实时计算
    /// 待 InventoryReservation 模块实现后替换为 SUM(quantity) - InvRes.total_reserved()
    pub async fn total_available(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal> {
        let row = if let Some(wh) = warehouse_id {
            sqlx::query(
                "SELECT COALESCE(SUM(quantity - reserved_qty), 0) as total FROM stock_ledger WHERE product_id = $1 AND warehouse_id = $2",
            )
            .bind(product_id)
            .bind(wh)
            .fetch_one(executor)
            .await?
        } else {
            sqlx::query(
                "SELECT COALESCE(SUM(quantity - reserved_qty), 0) as total FROM stock_ledger WHERE product_id = $1",
            )
            .bind(product_id)
            .fetch_one(executor)
            .await?
        };

        Ok(row.get("total"))
    }

    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>> {
        let offset = (page.saturating_sub(1)) * page_size;

        // 动态构建 WHERE 条件
        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx = 0u32;

        if filter.product_id.is_some() { param_idx += 1; conditions.push(format!("product_id = ${param_idx}")); }
        if filter.warehouse_id.is_some() { param_idx += 1; conditions.push(format!("warehouse_id = ${param_idx}")); }
        if filter.zone_id.is_some() { param_idx += 1; conditions.push(format!("zone_id = ${param_idx}")); }
        if filter.bin_id.is_some() { param_idx += 1; conditions.push(format!("bin_id = ${param_idx}")); }
        if filter.batch_no.is_some() { param_idx += 1; conditions.push(format!("batch_no = ${param_idx}")); }

        let where_sql = if conditions.is_empty() { String::new() } else { format!(" AND {}", conditions.join(" AND ")) };
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM stock_ledger WHERE 1=1{where_sql}");
        let data_sql = format!(
            "SELECT id, product_id, warehouse_id, zone_id, bin_id, batch_no, \
             quantity, reserved_qty, available_qty, unit_cost, received_date, expiry_date, updated_at \
             FROM stock_ledger WHERE 1=1{where_sql} \
             ORDER BY updated_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        // 构建并执行 count 查询
        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some(v) = filter.product_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.warehouse_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.zone_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.bin_id { count_q = count_q.bind(v); }
        if let Some(ref v) = filter.batch_no { count_q = count_q.bind(v); }

        // 构建并执行 data 查询
        let mut data_q = sqlx::query(&data_sql);
        if let Some(v) = filter.product_id { data_q = data_q.bind(v); }
        if let Some(v) = filter.warehouse_id { data_q = data_q.bind(v); }
        if let Some(v) = filter.zone_id { data_q = data_q.bind(v); }
        if let Some(v) = filter.bin_id { data_q = data_q.bind(v); }
        if let Some(ref v) = filter.batch_no { data_q = data_q.bind(v); }
        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<StockLedger> = rows.iter().filter_map(|r| StockLedger::from_row(r).ok()).collect();

        let total_pages = (total as u64).div_ceil(page_size as u64) as u32;

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }

    /// Upsert stock quantity (set absolute value) — Excel import support
    pub async fn upsert_quantity(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: i64,
        zone_id: i64,
        bin_id: i64,
        quantity: Decimal,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, quantity, available_qty, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5, NOW())
            ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, ''))
            DO UPDATE SET quantity = $5, available_qty = $5, updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(zone_id)
        .bind(bin_id)
        .bind(quantity)
        .fetch_one(executor)
        .await?;

        Ok(row.get("id"))
    }

    /// Set safety stock for a location — Excel import support
    pub async fn set_safety_stock(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: i64,
        zone_id: i64,
        bin_id: i64,
        safety_stock: Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO stock_ledger (product_id, warehouse_id, zone_id, bin_id, quantity, available_qty, safety_stock, updated_at)
            VALUES ($1, $2, $3, $4, 0, 0, $5, NOW())
            ON CONFLICT (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, ''))
            DO UPDATE SET safety_stock = $5, updated_at = NOW()
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(zone_id)
        .bind(bin_id)
        .bind(safety_stock)
        .execute(executor)
        .await?;
        Ok(())
    }

    // ---- Excel 导出辅助方法 ----

    /// 列出所有库存数据用于 Excel 导出，关联产品/仓库/库区/储位/价格/分类信息
    pub async fn list_for_export(executor: &mut sqlx::postgres::PgConnection) -> Result<Vec<StockExportRow>> {
        let rows = sqlx::query_as::<sqlx::Postgres, StockExportRow>(
            r#"
            SELECT
                p.product_id, p.pdt_name, p.product_code,
                p.meta->>'specification' as specification, p.unit,
                w.name as warehouse_name, z.code as zone_code, b.code as bin_code,
                sl.quantity, sl.safety_stock,
                (SELECT new_price FROM price_log WHERE product_id = p.product_id AND price_type = 1 ORDER BY created_at DESC LIMIT 1) as price,
                (SELECT string_agg(pc.category_id::text, ',') FROM product_categories pc WHERE pc.product_id = p.product_id) as category_ids,
                (SELECT string_agg(c.category_name, ',') FROM product_categories pc JOIN categories c ON pc.category_id = c.category_id WHERE pc.product_id = p.product_id) as category_names
            FROM products p
            LEFT JOIN stock_ledger sl ON p.product_id = sl.product_id
            LEFT JOIN bins b ON sl.bin_id = b.id
            LEFT JOIN zones z ON b.zone_id = z.id
            LEFT JOIN warehouses w ON z.warehouse_id = w.id
            WHERE p.deleted_at IS NULL
            ORDER BY p.product_id
            "#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 查询没有价格记录的产品（用于 Excel 导入校验提示）
    pub async fn find_products_without_price(executor: &mut sqlx::postgres::PgConnection) -> Result<Vec<ProductWithoutPriceRow>> {
        let rows = sqlx::query_as::<sqlx::Postgres, ProductWithoutPriceRow>(
            r#"
            SELECT p.product_id, p.pdt_name, p.product_code, p.unit, p.meta->>'specification' as specification
            FROM products p
            WHERE p.deleted_at IS NULL
              AND NOT EXISTS (SELECT 1 FROM price_log pl WHERE pl.product_id = p.product_id)
            ORDER BY p.product_id
            "#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}
