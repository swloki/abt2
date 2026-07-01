//! 采购作业中心聚合视图查询（跨表只读，统计下推 SQL）。
//!
//! 替代 `summary` 的「拉 `RECEIVING_SCAN_SIZE` 到内存按日期 filter 计逾期/临期」
//! 与 `pending_returns` 的「拉 200 条 `.iter().map().sum()` 算金额（>200 遗漏）」。

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{Postgres, QueryBuilder, Row};

use crate::shared::types::{PgExecutor, Result};

pub struct PurchaseWorkCenterRepo;

impl PurchaseWorkCenterRepo {
    /// 待收货 PO（Confirmed + PartiallyReceived）的逾期/临期计数。
    /// SQL `COUNT(*) FILTER`，避免拉 `RECEIVING_SCAN_SIZE` 到内存按日期 filter。
    /// `expected_delivery_date` 为 NULL 的不计入（与原内存逻辑一致）。
    pub async fn count_po_overdue_soon(
        db: PgExecutor<'_>,
        today: NaiveDate,
        soon_limit: NaiveDate,
    ) -> Result<(u64, u64)> {
        let mut qb = QueryBuilder::<Postgres>::new("");
        qb.push("SELECT COUNT(*) FILTER (WHERE expected_delivery_date < ");
        qb.push_bind(today);
        qb.push(") AS overdue, COUNT(*) FILTER (WHERE expected_delivery_date >= ");
        qb.push_bind(today);
        qb.push(" AND expected_delivery_date <= ");
        qb.push_bind(soon_limit);
        qb.push(") AS soon FROM purchase_orders \
                 WHERE deleted_at IS NULL AND status = ANY(");
        qb.push_bind(vec![2i16, 3]).push(")"); // Confirmed, PartiallyReceived
        let row = qb.build().fetch_one(&mut *db).await?;
        Ok((
            row.try_get::<i64, _>("overdue")? as u64,
            row.try_get::<i64, _>("soon")? as u64,
        ))
    }

    /// 某供应商指定状态退货的 (笔数, 金额合计)。
    /// SQL `COUNT + SUM`，避免拉 200 条算金额（>200 会遗漏）。
    pub async fn return_stats(
        db: PgExecutor<'_>,
        supplier_id: i64,
        status: i16,
    ) -> Result<(u64, Decimal)> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS cnt, COALESCE(SUM(total_amount), 0) AS amt \
             FROM purchase_returns \
             WHERE deleted_at IS NULL AND supplier_id = $1 AND status = $2",
        )
        .bind(supplier_id)
        .bind(status)
        .fetch_one(&mut *db)
        .await?;
        Ok((
            row.try_get::<i64, _>("cnt")? as u64,
            row.try_get::<Decimal, _>("amt")?,
        ))
    }
}
