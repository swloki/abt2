use rust_decimal::Decimal;
use sqlx::{FromRow, Row};
use crate::shared::types::Result;

use super::model::{InventoryReservation, ReserveRequest};
use crate::shared::enums::{DocumentType, ReservationStatus};

pub struct InventoryReservationRepo;

impl InventoryReservationRepo {
    /// 获取事务级 advisory lock，序列化同一 product 的所有并发预留（跨仓库）。
    ///
    /// 单 key bigint：双 key 版本只接受 `(int4,int4)`，与 i64 推断的 bigint 类型
    /// 不兼容（PostgreSQL 无 `pg_advisory_xact_lock(bigint,bigint)` 重载）。跨仓库
    /// 预留下也无法用 product+warehouse 双 key（warehouse 为 None）。锁 product 串行化
    /// 同产品所有预留，防超卖的正确性由 `available_atp` 校验保证。
    pub async fn lock_for_reserve(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
    ) -> Result<()> {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(product_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// INSERT 单条预留记录，返回生成的实体
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &ReserveRequest,
    ) -> Result<InventoryReservation> {
        let row = sqlx::query(
            r#"
            INSERT INTO inventory_reservations
                (product_id, warehouse_id, reserved_qty, reservation_type,
                 source_type, source_id, source_line_id, status, priority, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, product_id, warehouse_id, reserved_qty, reservation_type,
                      source_type, source_id, source_line_id, status, priority, expires_at, created_at
            "#,
        )
        .bind(req.product_id)
        .bind(req.warehouse_id)
        .bind(req.reserved_qty)
        .bind(req.reservation_type)
        .bind(req.source_type)
        .bind(req.source_id)
        .bind(req.source_line_id)
        .bind(ReservationStatus::Active)
        .bind(req.priority)
        .bind(req.expires_at)
        .fetch_one(&mut *executor)
        .await?;

        Ok(InventoryReservation::from_row(&row)?)
    }

    /// 履行预留：UPDATE status = Fulfilled WHERE id = $1 AND status = Active
    /// 返回受影响行数
    pub async fn fulfill(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $2
            WHERE id = $1 AND status = $3
            "#,
        )
        .bind(id)
        .bind(ReservationStatus::Fulfilled)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 取消预留：UPDATE status = Cancelled WHERE id = $1 AND status = Active
    /// 返回受影响行数
    pub async fn cancel(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $2
            WHERE id = $1 AND status = $3
            "#,
        )
        .bind(id)
        .bind(ReservationStatus::Cancelled)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 查询 Active 状态的预留总量
    /// warehouse_id 为 None 时汇总所有仓库
    pub async fn total_reserved(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(reserved_qty), 0) AS total
            FROM inventory_reservations
            WHERE product_id = $1
              AND ($2::bigint IS NULL OR warehouse_id = $2)
              AND status = $3
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_one(&mut *executor)
        .await?;

        let total: Decimal = row.try_get("total")?;
        Ok(total)
    }

    /// 按 product_id 查询 Active 预留明细（JOIN 销售订单 + 客户），用于前端「被占用」明细展示。
    /// warehouse_id 为 None 时跨所有仓库汇总。非销售订单来源的单号/客户为 None。
    pub async fn list_active_by_product(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<super::model::ReservationDetail>> {
        let items = sqlx::query_as::<_, super::model::ReservationDetail>(
            r#"
            SELECT ir.id, ir.reserved_qty, ir.reservation_type, ir.source_type,
                   ir.source_id, ir.source_line_id, ir.status, ir.created_at,
                   so.doc_number   AS source_doc_number,
                   so.status       AS source_status,
                   c.customer_name AS customer_name
            FROM inventory_reservations ir
            LEFT JOIN sales_orders so ON so.id = ir.source_id AND ir.source_type = 2
            LEFT JOIN customers c    ON c.customer_id = so.customer_id
            WHERE ir.product_id = $1
              AND ir.status = $3
              AND ($2::bigint IS NULL OR ir.warehouse_id = $2)
            ORDER BY ir.created_at DESC
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_all(&mut *executor)
        .await?;

        Ok(items)
    }

    /// 按来源单据查询每行的实际 Active 预留总量。
    /// 返回 HashMap<source_line_id, reserved_qty>，用于 confirm 后计算 shortage。
    pub async fn reserved_qty_by_source(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<std::collections::HashMap<i64, Decimal>> {
        let rows = sqlx::query(
            r#"
            SELECT source_line_id, SUM(reserved_qty) AS qty
            FROM inventory_reservations
            WHERE source_type = $1 AND source_id = $2 AND status = $3
            GROUP BY source_line_id
            "#,
        )
        .bind(source_type)
        .bind(source_id)
        .bind(ReservationStatus::Active)
        .fetch_all(&mut *executor)
        .await?;

        let mut map = std::collections::HashMap::new();
        for row in rows {
            let line_id: Option<i64> = row.try_get("source_line_id")?;
            let qty: Decimal = row.try_get("qty")?;
            if let Some(id) = line_id {
                map.insert(id, qty);
            }
        }
        Ok(map)
    }

    /// 计算跨表综合可用量（ATP）：
    ///   SUM(stock_ledger.quantity - stock_ledger.reserved_qty)   扣除 inventory_lock 预留
    ///   - SUM(inventory_reservations.reserved_qty WHERE Active)  扣除本表预留
    ///
    /// 双重扣除是防超卖的关键——`stock_ledger.reserved_qty` 由 `wms/inventory_lock`
    /// 维护，本表预留独立记录，两者并存必须同时扣。warehouse_id 为 None 时跨所有
    /// 仓库汇总（按 product 维度 ATP）。
    ///
    /// 直接 SQL 读 stock_ledger，不 `use crate::wms`（避免 shared→wms 分层倒置）；
    /// 这是 `wms/stock_ledger/repo.rs` `total_available` 注释指向的设计——
    /// InventoryReservation 负责成为预留真相源、自行计算 ATP。
    pub async fn available_atp(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal> {
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(
                    (SELECT SUM(quantity - reserved_qty)
                     FROM stock_ledger
                     WHERE product_id = $1 AND ($2::bigint IS NULL OR warehouse_id = $2)
                    ), 0
                )
                - COALESCE(
                    (SELECT SUM(reserved_qty)
                     FROM inventory_reservations
                     WHERE product_id = $1
                       AND ($2::bigint IS NULL OR warehouse_id = $2)
                       AND status = $3
                    ), 0
                ) AS atp
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_one(executor)
        .await?;

        let atp: Decimal = row.try_get("atp")?;
        Ok(atp)
    }

    /// 批量 ATP（多 product 一次查询，消除 compute_availability 的 N+1）：
    /// stock_ledger 现货 - inventory_reservations 硬预留。未命中的 product ATP=0。
    pub async fn available_atp_batch(
        executor: &mut sqlx::postgres::PgConnection,
        product_ids: &[i64],
        warehouse_id: Option<i64>,
    ) -> Result<std::collections::HashMap<i64, Decimal>> {
        use sqlx::Row;
        use std::collections::HashMap;
        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let stock_rows = sqlx::query(
            r#"SELECT product_id, COALESCE(SUM(quantity - reserved_qty), 0) AS qty
               FROM stock_ledger
               WHERE product_id = ANY($1) AND ($2::bigint IS NULL OR warehouse_id = $2)
               GROUP BY product_id"#,
        )
        .bind(product_ids)
        .bind(warehouse_id)
        .fetch_all(&mut *executor)
        .await?;
        let mut stock: HashMap<i64, Decimal> = HashMap::new();
        for r in stock_rows {
            stock.insert(r.try_get("product_id")?, r.try_get("qty")?);
        }
        let resv_rows = sqlx::query(
            r#"SELECT product_id, COALESCE(SUM(reserved_qty), 0) AS qty
               FROM inventory_reservations
               WHERE product_id = ANY($1) AND ($2::bigint IS NULL OR warehouse_id = $2) AND status = $3
               GROUP BY product_id"#,
        )
        .bind(product_ids)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_all(&mut *executor)
        .await?;
        let mut resv: HashMap<i64, Decimal> = HashMap::new();
        for r in resv_rows {
            resv.insert(r.try_get("product_id")?, r.try_get("qty")?);
        }
        let mut result: HashMap<i64, Decimal> = HashMap::new();
        for &pid in product_ids {
            let s = stock.get(&pid).copied().unwrap_or(Decimal::ZERO);
            let r = resv.get(&pid).copied().unwrap_or(Decimal::ZERO);
            result.insert(pid, s - r);
        }
        Ok(result)
    }

    /// 按来源取消全部 Active 预留
    pub async fn cancel_by_source(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $1
            WHERE source_type = $2 AND source_id = $3 AND status = $4
            "#,
        )
        .bind(ReservationStatus::Cancelled)
        .bind(source_type)
        .bind(source_id)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 按来源行履行 Active 预留
    pub async fn fulfill_by_source_line(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: DocumentType,
        source_line_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $1
            WHERE source_type = $2 AND source_line_id = $3 AND status = $4
            "#,
        )
        .bind(ReservationStatus::Fulfilled)
        .bind(source_type)
        .bind(source_line_id)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }
    /// 消耗预留 — 扣减指定来源+产品的预留量，归零时标记 Fulfilled
    pub async fn consume(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: DocumentType,
        source_id: i64,
        product_id: i64,
        qty: Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE inventory_reservations
               SET reserved_qty = reserved_qty - $4,
                   status = CASE WHEN reserved_qty - $4 <= 0 THEN $5 ELSE status END
               WHERE source_type = $1 AND source_id = $2 AND product_id = $3
                 AND status = $6 AND reserved_qty >= $4"#,
        )
        .bind(source_type)
        .bind(source_id)
        .bind(product_id)
        .bind(qty)
        .bind(ReservationStatus::Fulfilled)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
}
