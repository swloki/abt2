//! WMS 作业中心聚合视图查询（跨域只读，dashboard 性质）。
//!
//! 直接查各业务域表（pick_lists / shipping_requests / ...）实现**数据库分页** +
//! 轻量 count，替代旧 `fetch_domain_tasks` 的「拉 FETCH_LIMIT 全量再内存分页」。
//! 各业务域 service 不受影响，仍供各自列表页使用。

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use sqlx::{Postgres, QueryBuilder, Row};

use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::shared::types::{PgExecutor, Result};

use super::model::{OutboundStage, PendingTask, PendingTaskFilter, TaskSourceKind, Urgency, WorkCenterDomain};

/// 临期阈值（today + N 天内 = Soon）。
const SOON_DAYS: i64 = 2;

pub struct WorkCenterRepo;

/// 单实体域查询配置（Arrival 走 UNION，单独处理；Outbound 带 pick_lists JOIN 算拣货阶段）。
struct SimpleDomainCfg {
    table: &'static str,
    statuses: &'static [i16],
    /// SELECT 展示用 expected（统一转 date）
    expected_display: &'static str,
    /// urgency CASE + ORDER BY 用（日期列）
    expected_urgency: &'static str,
    has_deleted_at: bool,
    join: &'static str,
    /// 额外 JOIN（仅 Outbound：pick_lists，算拣货阶段）。其他域空串
    extra_join: &'static str,
    /// 额外 SELECT 列（仅 Outbound：stage CASE + pick_list_id），追加在 urgency_rank 后。其他域空串
    extra_select: &'static str,
    counterparty: &'static str,
    summary: &'static str,
    /// 额外 WHERE 片段（仅 Requisition：picking_type 过滤）。其他域空串
    extra_where: &'static str,
}

fn simple_cfg(domain: WorkCenterDomain) -> SimpleDomainCfg {
    match domain {
        WorkCenterDomain::Outbound => SimpleDomainCfg {
            table: "shipping_requests",
            statuses: &[2, 3], // Confirmed, Picking
            expected_display: "t.expected_ship_date",
            expected_urgency: "t.expected_ship_date",
            has_deleted_at: true,
            join: "LEFT JOIN customers c ON c.customer_id = t.customer_id AND c.deleted_at IS NULL",
            // 待出库合并（2026-07）：JOIN 拣货单算阶段，驱动就地拣货/发货分发
            extra_join: "LEFT JOIN pick_lists pl ON pl.outbound_id = t.id AND pl.deleted_at IS NULL",
            extra_select: ", CASE WHEN t.status = 2 THEN 'Unpicked' WHEN pl.status = 1 THEN 'Picking' WHEN pl.status = 2 THEN 'ReadyToShip' ELSE 'Unpicked' END AS stage, pl.id AS pick_list_id",
            counterparty: "c.customer_name",
            summary: "'待出库'",
            extra_where: "",
        },
        WorkCenterDomain::Requisition => SimpleDomainCfg {
            table: "stock_pickings",
            statuses: &[2], // Confirmed（部分发料 picking 仍 Confirmed）
            expected_display: "t.scheduled_date",
            expected_urgency: "t.scheduled_date",
            has_deleted_at: true,
            join: "LEFT JOIN work_orders wo ON wo.id = t.work_order_id",
            extra_join: "",
            extra_select: "",
            counterparty: "wo.doc_number",
            summary: "'领料'",
            extra_where: " AND t.picking_type = 5", // InternalIssue
        },
        WorkCenterDomain::Transfer => SimpleDomainCfg {
            table: "stock_pickings",
            statuses: &[1, 2], // Draft(待调出), Confirmed(在途)
            expected_display: "t.scheduled_date",
            expected_urgency: "t.scheduled_date",
            has_deleted_at: true,
            join: "LEFT JOIN warehouses wf ON wf.id = t.from_warehouse_id \
                   LEFT JOIN warehouses wt ON wt.id = t.to_warehouse_id",
            extra_join: "",
            extra_select: "",
            counterparty: "(wf.name || '→' || wt.name)",
            summary: "'调拨'",
            extra_where: " AND t.picking_type = 4", // InternalTransfer
        },
        WorkCenterDomain::CycleCount => SimpleDomainCfg {
            table: "cycle_counts",
            statuses: &[1, 2, 6], // Draft, Counting, PendingReview
            expected_display: "t.count_date",
            expected_urgency: "t.count_date",
            has_deleted_at: false,
            join: "LEFT JOIN warehouses w ON w.id = t.warehouse_id",
            extra_join: "",
            extra_select: "",
            counterparty: "w.name",
            summary: "'盘点'",
            extra_where: "",
        },
        WorkCenterDomain::Arrival => unreachable!("Arrival 走 UNION 单独处理"),
    }
}

/// 把 urgency CASE 片段推入 QueryBuilder（单一日期口径）：
/// `CASE WHEN exp < $today THEN 2 WHEN exp <= $today_soon THEN 1 ELSE 0 END`（today/today_soon push_bind）
fn push_urgency_case(qb: &mut QueryBuilder<Postgres>, cfg: &SimpleDomainCfg, today: NaiveDate) {
    let today_soon = today + chrono::Duration::days(SOON_DAYS);
    qb.push(" CASE WHEN ").push(cfg.expected_urgency).push(" < ");
    qb.push_bind(today).push(" THEN 2 WHEN ").push(cfg.expected_urgency).push(" <= ");
    qb.push_bind(today_soon).push(" THEN 1 ELSE 0 END");
}

impl WorkCenterRepo {
    /// 该域全量统计（summary 用）：(total, overdue, soon)。不含 keyword/urgency filter。
    pub async fn count_domain(
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        today: NaiveDate,
    ) -> Result<(u64, u64, u64)> {
        if domain == WorkCenterDomain::Arrival {
            return Self::count_arrival(db, today).await;
        }
        let cfg = simple_cfg(domain);
        let mut qb = QueryBuilder::<Postgres>::new("");
        qb.push("SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE ");
        push_urgency_case(&mut qb, &cfg, today);
        qb.push(" = 2) AS overdue, COUNT(*) FILTER (WHERE ");
        push_urgency_case(&mut qb, &cfg, today);
        qb.push(" = 1) AS soon FROM ").push(cfg.table).push(" t ").push(cfg.join).push(" WHERE ");
        if cfg.has_deleted_at {
            qb.push("t.deleted_at IS NULL AND ");
        }
        qb.push("t.status = ANY(").push_bind(cfg.statuses.to_vec()).push(")").push(cfg.extra_where);

        let row = qb.build().fetch_one(&mut *db).await?;
        let total = row.try_get::<i64, _>("total")? as u64;
        let overdue = row.try_get::<i64, _>("overdue")? as u64;
        let soon = row.try_get::<i64, _>("soon")? as u64;
        Ok((total, overdue, soon))
    }

    /// 该域待办队列（数据库分页 + keyword/urgency filter）。
    pub async fn list_domain_page(
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        filter: &PendingTaskFilter,
        today: NaiveDate,
        page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>> {
        let total = Self::count_domain_filtered(db, domain, filter, today).await?;
        if domain == WorkCenterDomain::Arrival {
            let items = Self::list_arrival_page(db, filter, today, &page).await?;
            return Ok(PaginatedResult::new(items, total, page.page, page.page_size));
        }
        let cfg = simple_cfg(domain);
        let page_size = page.page_size as i64;
        let offset = ((page.page.max(1) - 1) * page.page_size) as i64;

        // 子查询：算 urgency_rank + 取原始列（Outbound 额外带 stage + pick_list_id）；外层：keyword/urgency filter + 排序 + 分页
        let is_outbound = domain == WorkCenterDomain::Outbound;
        let mut qb = QueryBuilder::<Postgres>::new("");
        qb.push("SELECT x.id, x.doc_number, x.counterparty, x.summary, x.expected_at, x.received_at, x.urgency_rank");
        if is_outbound {
            qb.push(", x.stage, x.pick_list_id");
        }
        qb.push(" FROM (SELECT t.id, t.doc_number, ").push(cfg.counterparty).push(" AS counterparty, ");
        qb.push(cfg.summary).push(" AS summary, ").push(cfg.expected_display).push(" AS expected_at, t.created_at AS received_at, ");
        push_urgency_case(&mut qb, &cfg, today);
        qb.push(" AS urgency_rank").push(cfg.extra_select);
        qb.push(" FROM ").push(cfg.table).push(" t ").push(cfg.join).push(" ").push(cfg.extra_join).push(" WHERE ");
        if cfg.has_deleted_at {
            qb.push("t.deleted_at IS NULL AND ");
        }
        qb.push("t.status = ANY(").push_bind(cfg.statuses.to_vec()).push(")").push(cfg.extra_where);
        qb.push(") x WHERE ");
        // keyword（匹配单号或 counterparty）
        if let Some(kw) = filter.keyword.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let pat = format!("%{kw}%");
            qb.push("(x.doc_number ILIKE ").push_bind(pat.clone());
            qb.push(" OR x.counterparty ILIKE ").push_bind(pat).push(") AND ");
        }
        // urgency filter
        if let Some(u) = filter.urgency {
            qb.push("x.urgency_rank = ").push_bind(urgency_rank_val(u) as i32).push(" AND ");
        }
        qb.push("TRUE ORDER BY x.urgency_rank DESC, x.expected_at ASC NULLS LAST LIMIT ");
        qb.push_bind(page_size).push(" OFFSET ").push_bind(offset);

        let rows = qb.build().fetch_all(&mut *db).await?;
        let items = rows
            .iter()
            .map(|r| map_simple_row(r, domain))
            .collect::<Result<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    /// 该域过滤后总数（list_domain_page 分页用，含 keyword/urgency filter）。
    async fn count_domain_filtered(
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        filter: &PendingTaskFilter,
        today: NaiveDate,
    ) -> Result<u64> {
        if domain == WorkCenterDomain::Arrival {
            return Self::count_arrival_filtered(db, filter, today).await;
        }
        let cfg = simple_cfg(domain);
        let mut qb = QueryBuilder::<Postgres>::new("");
        qb.push("SELECT COUNT(*) FROM (SELECT t.id, ").push(cfg.counterparty).push(" AS counterparty, ");
        push_urgency_case(&mut qb, &cfg, today);
        qb.push(" AS urgency_rank, t.doc_number FROM ").push(cfg.table).push(" t ").push(cfg.join).push(" WHERE ");
        if cfg.has_deleted_at {
            qb.push("t.deleted_at IS NULL AND ");
        }
        qb.push("t.status = ANY(").push_bind(cfg.statuses.to_vec()).push(")").push(cfg.extra_where);
        qb.push(") x WHERE ");
        if let Some(kw) = filter.keyword.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let pat = format!("%{kw}%");
            qb.push("(x.doc_number ILIKE ").push_bind(pat.clone());
            qb.push(" OR x.counterparty ILIKE ").push_bind(pat).push(") AND ");
        }
        if let Some(u) = filter.urgency {
            qb.push("x.urgency_rank = ").push_bind(urgency_rank_val(u) as i32).push(" AND ");
        }
        qb.push("TRUE");

        let total = qb.build_query_scalar::<i64>().fetch_one(&mut *db).await? as u64;
        Ok(total)
    }

    // ── Arrival（PO + WO UNION ALL）──

    /// Arrival 全量统计（summary 用）。
    async fn count_arrival(db: PgExecutor<'_>, today: NaiveDate) -> Result<(u64, u64, u64)> {
        let today_soon = today + chrono::Duration::days(SOON_DAYS);
        let mut qb = QueryBuilder::<Postgres>::new("");
        qb.push("SELECT COUNT(*) AS total, \
                 COUNT(*) FILTER (WHERE u.urgency_rank = 2) AS overdue, \
                 COUNT(*) FILTER (WHERE u.urgency_rank = 1) AS soon FROM (");
        // PO 子查询
        qb.push("SELECT CASE WHEN po.expected_delivery_date < ").push_bind(today);
        qb.push(" THEN 2 WHEN po.expected_delivery_date <= ").push_bind(today_soon);
        qb.push(" THEN 1 ELSE 0 END AS urgency_rank FROM purchase_orders po \
                 WHERE po.deleted_at IS NULL AND po.status = ANY(").push_bind(vec![2i16, 3]).push(")");
        // WO 子查询
        qb.push(" UNION ALL SELECT CASE WHEN wo.scheduled_end < ").push_bind(today);
        qb.push(" THEN 2 WHEN wo.scheduled_end <= ").push_bind(today_soon);
        qb.push(" THEN 1 ELSE 0 END AS urgency_rank FROM work_orders wo \
                 LEFT JOIN (SELECT source_id, SUM(quantity) AS received FROM inventory_transactions \
                            WHERE source_type = 'work_order' GROUP BY source_id) it ON it.source_id = wo.id \
                 WHERE wo.deleted_at IS NULL AND wo.status = ANY(").push_bind(vec![3i16, 6]).push(") \
                 AND wo.completed_qty > COALESCE(it.received, 0)) u");

        let row = qb.build().fetch_one(&mut *db).await?;
        let total = row.try_get::<i64, _>("total")? as u64;
        let overdue = row.try_get::<i64, _>("overdue")? as u64;
        let soon = row.try_get::<i64, _>("soon")? as u64;
        Ok((total, overdue, soon))
    }

    /// Arrival 过滤后总数（分页用）。
    async fn count_arrival_filtered(
        db: PgExecutor<'_>,
        filter: &PendingTaskFilter,
        today: NaiveDate,
    ) -> Result<u64> {
        let today_soon = today + chrono::Duration::days(SOON_DAYS);
        let mut qb = QueryBuilder::<Postgres>::new("");
        qb.push("SELECT COUNT(*) FROM (");
        // PO 子查询
        qb.push("SELECT po.id, 'PurchaseOrder' AS source_kind, po.doc_number, \
                 s.supplier_name AS counterparty, '采购待收' AS summary, \
                 po.expected_delivery_date AS expected_at, po.created_at AS received_at, \
                 CASE WHEN po.expected_delivery_date < ");
        qb.push_bind(today).push(" THEN 2 WHEN po.expected_delivery_date <= ").push_bind(today_soon);
        qb.push(" THEN 1 ELSE 0 END AS urgency_rank FROM purchase_orders po \
                 LEFT JOIN suppliers s ON s.supplier_id = po.supplier_id AND s.deleted_at IS NULL \
                 WHERE po.deleted_at IS NULL AND po.status = ANY(").push_bind(vec![2i16, 3]).push(")");
        // WO 子查询
        qb.push(" UNION ALL SELECT wo.id, 'WorkOrder' AS source_kind, wo.doc_number, \
                 p.pdt_name AS counterparty, '待入库' AS summary, \
                 wo.scheduled_end AS expected_at, wo.created_at AS received_at, \
                 CASE WHEN wo.scheduled_end < ");
        qb.push_bind(today).push(" THEN 2 WHEN wo.scheduled_end <= ").push_bind(today_soon);
        qb.push(" THEN 1 ELSE 0 END AS urgency_rank FROM work_orders wo \
                 LEFT JOIN products p ON p.product_id = wo.product_id AND p.deleted_at IS NULL \
                 LEFT JOIN (SELECT source_id, SUM(quantity) AS received FROM inventory_transactions \
                            WHERE source_type = 'work_order' GROUP BY source_id) it ON it.source_id = wo.id \
                 WHERE wo.deleted_at IS NULL AND wo.status = ANY(").push_bind(vec![3i16, 6]).push(") \
                 AND wo.completed_qty > COALESCE(it.received, 0)) x WHERE ");
        append_arrival_filter(&mut qb, filter);
        qb.push("TRUE");
        let total = qb.build_query_scalar::<i64>().fetch_one(&mut *db).await? as u64;
        Ok(total)
    }

    /// Arrival 分页查询。
    async fn list_arrival_page(
        db: PgExecutor<'_>,
        filter: &PendingTaskFilter,
        today: NaiveDate,
        page: &PageParams,
    ) -> Result<Vec<PendingTask>> {
        let today_soon = today + chrono::Duration::days(SOON_DAYS);
        let page_size = page.page_size as i64;
        let offset = ((page.page.max(1) - 1) * page.page_size) as i64;
        let mut qb = QueryBuilder::<Postgres>::new("");
        qb.push("SELECT x.* FROM (");
        // PO
        qb.push("SELECT po.id, 'PurchaseOrder' AS source_kind, po.doc_number, \
                 s.supplier_name AS counterparty, '采购待收' AS summary, \
                 po.expected_delivery_date AS expected_at, po.created_at AS received_at, \
                 CASE WHEN po.expected_delivery_date < ");
        qb.push_bind(today).push(" THEN 2 WHEN po.expected_delivery_date <= ").push_bind(today_soon);
        qb.push(" THEN 1 ELSE 0 END AS urgency_rank FROM purchase_orders po \
                 LEFT JOIN suppliers s ON s.supplier_id = po.supplier_id AND s.deleted_at IS NULL \
                 WHERE po.deleted_at IS NULL AND po.status = ANY(").push_bind(vec![2i16, 3]).push(")");
        // WO
        qb.push(" UNION ALL SELECT wo.id, 'WorkOrder' AS source_kind, wo.doc_number, \
                 p.pdt_name AS counterparty, '待入库' AS summary, \
                 wo.scheduled_end AS expected_at, wo.created_at AS received_at, \
                 CASE WHEN wo.scheduled_end < ");
        qb.push_bind(today).push(" THEN 2 WHEN wo.scheduled_end <= ").push_bind(today_soon);
        qb.push(" THEN 1 ELSE 0 END AS urgency_rank FROM work_orders wo \
                 LEFT JOIN products p ON p.product_id = wo.product_id AND p.deleted_at IS NULL \
                 LEFT JOIN (SELECT source_id, SUM(quantity) AS received FROM inventory_transactions \
                            WHERE source_type = 'work_order' GROUP BY source_id) it ON it.source_id = wo.id \
                 WHERE wo.deleted_at IS NULL AND wo.status = ANY(").push_bind(vec![3i16, 6]).push(") \
                 AND wo.completed_qty > COALESCE(it.received, 0)) x WHERE ");
        append_arrival_filter(&mut qb, filter);
        qb.push("TRUE ORDER BY x.urgency_rank DESC, x.expected_at ASC NULLS LAST LIMIT ");
        qb.push_bind(page_size).push(" OFFSET ").push_bind(offset);

        let rows = qb.build().fetch_all(&mut *db).await?;
        rows.iter().map(map_arrival_row).collect()
    }
}

/// Arrival 外层过滤条件（keyword + urgency + source_kind）。
fn append_arrival_filter(qb: &mut QueryBuilder<Postgres>, filter: &PendingTaskFilter) {
    if let Some(kw) = filter.keyword.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let pat = format!("%{kw}%");
        qb.push("(x.doc_number ILIKE ").push_bind(pat.clone());
        qb.push(" OR x.counterparty ILIKE ").push_bind(pat).push(") AND ");
    }
    if let Some(u) = filter.urgency {
        qb.push("x.urgency_rank = ").push_bind(urgency_rank_val(u) as i32).push(" AND ");
    }
    if let Some(sk) = filter.source_kind {
        let kind = match sk {
            TaskSourceKind::PurchaseOrder => "PurchaseOrder",
            TaskSourceKind::WorkOrder => "WorkOrder",
        };
        qb.push("x.source_kind = ").push_bind(kind).push(" AND ");
    }
}

fn urgency_rank_val(u: Urgency) -> u8 {
    match u {
        Urgency::Overdue => 2,
        Urgency::Soon => 1,
        Urgency::Normal => 0,
    }
}

fn urgency_from_rank(rank: i32) -> Urgency {
    match rank {
        2 => Urgency::Overdue,
        1 => Urgency::Soon,
        _ => Urgency::Normal,
    }
}

/// 单实体域行映射（source_kind 占位 PurchaseOrder，仅 Arrival 有意义）
fn map_simple_row(r: &sqlx::postgres::PgRow, domain: WorkCenterDomain) -> Result<PendingTask> {
    let doc_id: i64 = r.try_get("id")?;
    let doc_number: String = r.try_get("doc_number")?;
    let counterparty: String = r.try_get::<Option<String>, _>("counterparty")?.unwrap_or_default();
    let summary: String = r.try_get("summary")?;
    let expected_at = r
        .try_get::<Option<NaiveDate>, _>("expected_at")?
        .map(midnight_utc);
    let urgency_rank: i32 = r.try_get("urgency_rank")?;
    let received_at = r.try_get::<Option<DateTime<Utc>>, _>("received_at")?;
    // Outbound 域：读拣货阶段 + pick_list_id（驱动前端就地拣货/发货分发）
    let (outbound_stage, pick_list_id) = if domain == WorkCenterDomain::Outbound {
        let stage = match r.try_get::<String, _>("stage")?.as_str() {
            "Picking" => OutboundStage::Picking,
            "ReadyToShip" => OutboundStage::ReadyToShip,
            _ => OutboundStage::Unpicked,
        };
        (Some(stage), r.try_get::<Option<i64>, _>("pick_list_id")?)
    } else {
        (None, None)
    };
    Ok(PendingTask {
        doc_id,
        doc_number,
        domain,
        source_kind: TaskSourceKind::PurchaseOrder, // 非 Arrival 占位
        counterparty,
        summary,
        expected_at,
        received_at,
        urgency: urgency_from_rank(urgency_rank),
        outbound_stage,
        pick_list_id,
    })
}

fn map_arrival_row(r: &sqlx::postgres::PgRow) -> Result<PendingTask> {
    let doc_id: i64 = r.try_get("id")?;
    let doc_number: String = r.try_get("doc_number")?;
    let source_kind = match r.try_get::<String, _>("source_kind")?.as_str() {
        "WorkOrder" => TaskSourceKind::WorkOrder,
        _ => TaskSourceKind::PurchaseOrder,
    };
    let counterparty: String = r.try_get::<Option<String>, _>("counterparty")?.unwrap_or_default();
    let summary: String = r.try_get("summary")?;
    let expected_at = r
        .try_get::<Option<NaiveDate>, _>("expected_at")?
        .map(midnight_utc);
    let urgency_rank: i32 = r.try_get("urgency_rank")?;
    let received_at = r.try_get::<Option<DateTime<Utc>>, _>("received_at")?;
    Ok(PendingTask {
        doc_id,
        doc_number,
        domain: WorkCenterDomain::Arrival,
        source_kind,
        counterparty,
        summary,
        expected_at,
        received_at,
        urgency: urgency_from_rank(urgency_rank),
        outbound_stage: None,
        pick_list_id: None,
    })
}

fn midnight_utc(d: NaiveDate) -> DateTime<Utc> {
    Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urgency_rank_roundtrip() {
        assert_eq!(urgency_rank_val(Urgency::Overdue), 2);
        assert_eq!(urgency_rank_val(Urgency::Soon), 1);
        assert_eq!(urgency_rank_val(Urgency::Normal), 0);
        assert_eq!(urgency_from_rank(2), Urgency::Overdue);
        assert_eq!(urgency_from_rank(1), Urgency::Soon);
        assert_eq!(urgency_from_rank(0), Urgency::Normal);
    }

    #[test]
    fn simple_cfg_statuses_correct() {
        assert_eq!(simple_cfg(WorkCenterDomain::Outbound).statuses, &[2, 3]);
        assert_eq!(simple_cfg(WorkCenterDomain::Requisition).statuses, &[2, 5]);
        assert_eq!(simple_cfg(WorkCenterDomain::Transfer).statuses, &[1, 2]);
        assert_eq!(simple_cfg(WorkCenterDomain::CycleCount).statuses, &[1, 2, 6]);
    }
}
