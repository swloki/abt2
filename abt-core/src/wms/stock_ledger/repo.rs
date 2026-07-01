use rust_decimal::Decimal;
use sqlx::{FromRow, Row};
use crate::shared::enums::ReservationStatus;
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

    /// 查询库位下当前被占用(quantity > 0)的「其他」产品（一库位一产品校验用）。
    /// 返回第一个冲突产品的 (product_id, product_name, quantity)；无冲突返回 None。
    pub async fn find_other_occupant_in_bin(
        executor: &mut sqlx::postgres::PgConnection,
        bin_id: i64,
        exclude_product_id: i64,
    ) -> Result<Option<(i64, String, Decimal)>> {
        let row = sqlx::query(
            r#"
            SELECT sl.product_id, p.pdt_name, sl.quantity
            FROM stock_ledger sl
            JOIN products p ON p.product_id = sl.product_id
            WHERE sl.bin_id = $1 AND sl.quantity > 0 AND sl.product_id <> $2
            LIMIT 1
            "#,
        )
        .bind(bin_id)
        .bind(exclude_product_id)
        .fetch_optional(executor)
        .await?;

        Ok(row.map(|r| {
            let pid: i64 = r.get("product_id");
            let name: String = r.get("pdt_name");
            let qty: Decimal = r.get("quantity");
            (pid, name, qty)
        }))
    }

    /// 查询指定 bin 是否已有该产品的正库存（同物料合并入库放行判断用）。
    /// 配合 find_other_occupant_in_bin 细化「一库位一产品」校验：
    /// 目标产品已在该 bin 有库存时允许继续入库（同物料合并），即使 bin 混放其他产品也不拒绝。
    pub async fn has_stock_in_bin(
        executor: &mut sqlx::postgres::PgConnection,
        bin_id: i64,
        product_id: i64,
    ) -> Result<bool> {
        let exists: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT 1::BIGINT FROM stock_ledger
            WHERE bin_id = $1 AND product_id = $2 AND quantity > 0
            LIMIT 1
            "#,
        )
        .bind(bin_id)
        .bind(product_id)
        .fetch_optional(executor)
        .await?;
        Ok(exists.is_some())
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

    /// 计算可用量 ATP = SUM(stock_ledger.quantity - stock_ledger.reserved_qty)
    ///                   - SUM(inventory_reservations.reserved_qty WHERE status=Active)
    ///
    /// 真相源说明：stock_ledger.reserved_qty 仅由 inventory_lock（WMS 物理锁）维护，
    /// 不反映 sales order 等业务在 inventory_reservations 表中的订单级预留。
    /// 因此可用量必须额外扣除 inventory_reservations 中的 Active 预留，否则
    /// 会出现「库存已被其他订单预留，详情页满足率仍显示 100%」的链路 Bug。
    pub async fn total_available(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal> {
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(sl.quantity - sl.reserved_qty), 0)
                - COALESCE((
                    SELECT SUM(ir.reserved_qty)
                    FROM inventory_reservations ir
                    WHERE ir.product_id = $1
                      AND ($2::bigint IS NULL OR ir.warehouse_id = $2)
                      AND ir.status = $3
                ), 0) AS total
            FROM stock_ledger sl
            WHERE sl.product_id = $1 AND ($2::bigint IS NULL OR sl.warehouse_id = $2)
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_one(executor)
        .await?;

        Ok(row.get("total"))
    }

    /// 批量查多个产品的可用量（GROUP BY product_id，避免逐个 total_available 的 N+1）。
    /// 返回 product_id → 可用量；无库存记录的产品不在 map 中（调用方按 0 处理）。口径与 `total_available` 一致。
    pub async fn total_available_batch(
        executor: &mut sqlx::postgres::PgConnection,
        product_ids: &[i64],
        warehouse_id: Option<i64>,
    ) -> Result<std::collections::HashMap<i64, Decimal>> {
        if product_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT sl.product_id,
                COALESCE(SUM(sl.quantity - sl.reserved_qty), 0)
                - COALESCE((
                    SELECT SUM(ir.reserved_qty)
                    FROM inventory_reservations ir
                    WHERE ir.product_id = sl.product_id
                      AND ($2::bigint IS NULL OR ir.warehouse_id = $2)
                      AND ir.status = $3
                ), 0) AS total
            FROM stock_ledger sl
            WHERE sl.product_id = ANY($1) AND ($2::bigint IS NULL OR sl.warehouse_id = $2)
            GROUP BY sl.product_id
            "#,
        )
        .bind(product_ids)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_all(executor)
        .await?;
        let mut map = std::collections::HashMap::with_capacity(rows.len());
        for r in &rows {
            map.insert(r.get::<i64, _>("product_id"), r.get::<Decimal, _>("total"));
        }
        Ok(map)
    }

    /// 预计可用量（参考 ERPNext bin.projected_qty 公式）
    ///
    /// 四维计算（子查询避免笛卡尔积）：
    /// 1. actual = SUM(stock_ledger.quantity)
    /// 2. on_order_po = SUM(poi.quantity - poi.received_qty) WHERE po.status IN (2,3)
    /// 3. in_progress_wo = SUM(wo.planned_qty - wo.completed_qty) WHERE wo.status IN (3,6)
    /// 4. reserved = SUM(inventory_reservations.reserved_qty WHERE Active)
    ///
    /// 注意：warehouse_id 仅过滤 actual 和 reserved；
    /// on_order_po 和 in_progress_wo 始终是全局值（PO/WO 表无仓库维度字段）。
    /// 当 warehouse_id=None 时（如 BOM 级联场景），所有维度一致。
    ///
    /// projected = actual + on_order_po + in_progress_wo - reserved
    pub async fn projected_qty(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<crate::wms::stock_ledger::service::ProjectedQty> {
        use crate::wms::stock_ledger::service::ProjectedQty;
        use rust_decimal::Decimal;

        let row = sqlx::query(
            r#"
            SELECT
                COALESCE((
                    SELECT SUM(sl.quantity) FROM stock_ledger sl
                    WHERE sl.product_id = $1 AND ($2::bigint IS NULL OR sl.warehouse_id = $2)
                ), 0) AS actual,
                COALESCE((
                    SELECT SUM(poi.quantity - poi.received_qty)
                    FROM purchase_order_items poi
                    JOIN purchase_orders po ON po.id = poi.order_id
                    WHERE poi.product_id = $1
                      AND po.status IN (2, 3)
                      AND po.deleted_at IS NULL
                ), 0) AS on_order_po,
                COALESCE((
                    SELECT SUM(wo.planned_qty - wo.completed_qty)
                    FROM work_orders wo
                    WHERE wo.product_id = $1
                      AND wo.status IN (3, 6)
                      AND wo.deleted_at IS NULL
                ), 0) AS in_progress_wo,
                COALESCE((
                    SELECT SUM(ir.reserved_qty)
                    FROM inventory_reservations ir
                    WHERE ir.product_id = $1
                      AND ($2::bigint IS NULL OR ir.warehouse_id = $2)
                      AND ir.status = $3
                ), 0) AS reserved
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_one(executor)
        .await?;

        let actual: Decimal = row.get("actual");
        let on_order_po: Decimal = row.get("on_order_po");
        let in_progress_wo: Decimal = row.get("in_progress_wo");
        let reserved: Decimal = row.get("reserved");
        let projected = actual + on_order_po + in_progress_wo - reserved;

        Ok(ProjectedQty { actual, on_order_po, in_progress_wo, reserved, projected })
    }

    /// 批量查询预计可用量（消除 N+1 查询）
    pub async fn projected_qty_batch(
        executor: &mut sqlx::postgres::PgConnection,
        product_ids: &[i64],
        warehouse_id: Option<i64>,
    ) -> Result<std::collections::HashMap<i64, crate::wms::stock_ledger::service::ProjectedQty>> {
        use crate::wms::stock_ledger::service::ProjectedQty;
        use rust_decimal::Decimal;
        use sqlx::Row;

        if product_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let rows = sqlx::query(
            r#"
            WITH pids AS (SELECT unnest($1::bigint[]) AS product_id)
            SELECT p.product_id,
                COALESCE((
                    SELECT SUM(sl.quantity) FROM stock_ledger sl
                    WHERE sl.product_id = p.product_id AND ($2::bigint IS NULL OR sl.warehouse_id = $2)
                ), 0) AS actual,
                COALESCE((
                    SELECT SUM(poi.quantity - poi.received_qty)
                    FROM purchase_order_items poi
                    JOIN purchase_orders po ON po.id = poi.order_id
                    WHERE poi.product_id = p.product_id
                      AND po.status IN (2, 3)
                      AND po.deleted_at IS NULL
                ), 0) AS on_order_po,
                COALESCE((
                    SELECT SUM(wo.planned_qty - wo.completed_qty)
                    FROM work_orders wo
                    WHERE wo.product_id = p.product_id
                      AND wo.status IN (3, 6)
                      AND wo.deleted_at IS NULL
                ), 0) AS in_progress_wo,
                COALESCE((
                    SELECT SUM(ir.reserved_qty)
                    FROM inventory_reservations ir
                    WHERE ir.product_id = p.product_id
                      AND ($2::bigint IS NULL OR ir.warehouse_id = $2)
                      AND ir.status = $3
                ), 0) AS reserved
            FROM pids p
            "#,
        )
        .bind(product_ids)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_all(executor)
        .await?;

        let mut map = std::collections::HashMap::new();
        for row in rows {
            let pid: i64 = row.try_get("product_id")?;
            let actual: Decimal = row.try_get("actual")?;
            let on_order_po: Decimal = row.try_get("on_order_po")?;
            let in_progress_wo: Decimal = row.try_get("in_progress_wo")?;
            let reserved: Decimal = row.try_get("reserved")?;
            let projected = actual + on_order_po + in_progress_wo - reserved;
            map.insert(pid, ProjectedQty { actual, on_order_po, in_progress_wo, reserved, projected });
        }
        Ok(map)
    }

    /// 产品最后已知单位成本（stock_ledger 最新一条有效 unit_cost，无则 0）
    pub async fn last_known_unit_cost(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
    ) -> Result<Decimal> {
        let cost: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(
                (SELECT unit_cost FROM stock_ledger
                 WHERE product_id = $1 AND unit_cost IS NOT NULL AND unit_cost > 0
                 ORDER BY id DESC LIMIT 1),
                0::numeric
            )"#,
        )
        .bind(product_id)
        .fetch_one(executor)
        .await?;
        Ok(cost)
    }

    /// 批量查询产品最后已知单位成本（消除 N+1）
    /// 返回 HashMap<product_id, unit_cost>，仅含存在有效成本的 product
    pub async fn last_known_unit_cost_batch(
        executor: &mut sqlx::postgres::PgConnection,
        product_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Decimal>> {
        if product_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let rows: Vec<(i64, Decimal)> = sqlx::query_as(
            r#"SELECT DISTINCT ON (product_id) product_id, unit_cost
               FROM stock_ledger
               WHERE product_id = ANY($1) AND unit_cost IS NOT NULL AND unit_cost > 0
               ORDER BY product_id, created_at DESC"#,
        )
        .bind(product_ids)
        .fetch_all(executor)
        .await?;
        Ok(rows.into_iter().collect())
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
        if filter.product_ids.as_ref().is_some_and(|ids| !ids.is_empty()) {
            param_idx += 1; conditions.push(format!("product_id = ANY(${param_idx})"));
        }
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
        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = filter.product_id { count_q = count_q.bind(v); }
        if let Some(ref v) = filter.product_ids { count_q = count_q.bind(v); }
        if let Some(v) = filter.warehouse_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.zone_id { count_q = count_q.bind(v); }
        if let Some(v) = filter.bin_id { count_q = count_q.bind(v); }
        if let Some(ref v) = filter.batch_no { count_q = count_q.bind(v); }

        // 构建并执行 data 查询
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = filter.product_id { data_q = data_q.bind(v); }
        if let Some(ref v) = filter.product_ids { data_q = data_q.bind(v); }
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

    /// 调整预留量：按产品+仓库增加/减少 reserved_qty（同步调整 available_qty）
    /// delta > 0 表示增加预留，delta < 0 表示释放预留
    pub async fn adjust_reserved_qty(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: i64,
        delta: rust_decimal::Decimal,
    ) -> Result<u64> {
        // 修复：原实现把 delta 加到 product×warehouse 的【每一行】，多库位时锁 N 会预留 N×行数。
        // query_available 用 SUM(quantity − reserved_qty)，故只需让 SUM 变化 delta 即正确。
        // 这里只更新 FIFO 首行（最早入库且有余量的库位），release 用同一行 −delta 对称回滚。
        let result = sqlx::query(
            r#"
            UPDATE stock_ledger
            SET reserved_qty = reserved_qty + $3,
                available_qty = available_qty - $3,
                updated_at = NOW()
            WHERE product_id = $1 AND warehouse_id = $2
              AND id = (
                  SELECT id FROM stock_ledger
                  WHERE product_id = $1 AND warehouse_id = $2
                  ORDER BY received_date ASC NULLS LAST, id ASC
                  LIMIT 1
              )
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(delta)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 解析默认库位（zone_id, bin_id）：优先取该产品在本仓有库存的 FIFO 首库位；
    /// 无库存则取本仓任一库位。用于 record() 在 zone/bin 缺失时仍能更新台账。
    pub async fn resolve_default_bin(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: i64,
    ) -> Result<Option<(i64, i64)>> {
        let row = sqlx::query(
            r#"
            SELECT zone_id, bin_id FROM (
                (SELECT zone_id, bin_id, 1 AS prio
                 FROM stock_ledger
                 WHERE product_id = $1 AND warehouse_id = $2 AND quantity > 0
                 ORDER BY received_date ASC NULLS LAST, id ASC
                 LIMIT 1)
                UNION ALL
                (SELECT b.zone_id AS zone_id, b.id AS bin_id, 2 AS prio
                 FROM bins b JOIN zones z ON b.zone_id = z.id
                 WHERE z.warehouse_id = $2
                 ORDER BY b.id ASC
                 LIMIT 1)
            ) combined
            ORDER BY prio ASC, bin_id ASC
            LIMIT 1
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .fetch_optional(&mut *executor)
        .await?;

        use sqlx::Row;
        Ok(row.map(|r| {
            let z: i64 = r.get("zone_id");
            let b: i64 = r.get("bin_id");
            (z, b)
        }))
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
