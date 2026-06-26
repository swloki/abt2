use async_trait::async_trait;
use chrono::{DateTime, Days, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{PendingTask, TaskSourceKind, Urgency, UrgentSummary, WorkCenterDomain, WorkCenterSummary};
use super::service::WorkCenterService;
use crate::shared::types::pagination::PageParams;
use crate::shared::types::{PaginatedResult, PgExecutor, Result, ServiceContext};
use crate::wms::cycle_count::{
    model::CycleCountFilter, new_cycle_count_service, service::CycleCountService,
};
use crate::wms::enums::{CycleCountStatus, RequisitionStatus, TransferStatus};
use crate::wms::material_requisition::{
    model::RequisitionFilter, new_material_requisition_service, service::MaterialRequisitionService,
};
use crate::wms::outbound::{
    model::{ShippingQuery, ShippingStatus}, new_shipping_request_service, service::ShippingRequestService,
};
use crate::wms::pick_list::{
    model::{PickListQuery, PickListStatus}, new_pick_list_service, service::PickListService,
};
use crate::wms::transfer::{model::TransferFilter, new_transfer_service, service::TransferService};
use crate::master_data::customer::{model::CustomerQuery, new_customer_service, CustomerService};
use crate::master_data::supplier::{model::SupplierQuery, new_supplier_service, SupplierService};
use crate::wms::warehouse::{model::WarehouseFilter, new_warehouse_service, WarehouseService};
use crate::mes::work_order::{model::WorkOrderFilter, new_work_order_service, WorkOrderService};
use crate::mes::WorkOrderStatus;
use crate::purchase::enums::PurchaseOrderStatus;
use crate::purchase::order::{model::PurchaseOrderQuery, new_purchase_order_service, PurchaseOrderService};
use crate::master_data::product::{new_product_service, ProductService};
use crate::wms::inventory_transaction::{new_inventory_transaction_service, InventoryTransactionService};
use std::collections::HashMap;

/// 临期阈值：today + N 天内到期视为 `Soon`。MVP 硬编码，后续进 wms/settings。
const SOON_DAYS: u64 = 2;
/// 拣货超时阈值：创建超过 N 小时视为 `Overdue`（拣货无到期日，用创建时长判超时）。
const PICK_TIMEOUT_HOURS: i64 = 4;
/// 单域拉取上限（与 `PageParams::page_size` clamp 上限对齐）。MVP：pending 超过此值的尾部不展示。
const FETCH_LIMIT: u32 = 200;

const ALL_DOMAINS: [WorkCenterDomain; 6] = [
    WorkCenterDomain::Arrival,
    WorkCenterDomain::Pick,
    WorkCenterDomain::Outbound,
    WorkCenterDomain::Requisition,
    WorkCenterDomain::Transfer,
    WorkCenterDomain::CycleCount,
];

pub struct WorkCenterServiceImpl {
    pool: PgPool,
}

impl WorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 拉取某环节 pending 单据并映射成 `PendingTask`（查询失败容错返回空 vec，不连累整页）。
    async fn fetch_domain_tasks(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        today: NaiveDate,
        now: DateTime<Utc>,
    ) -> Vec<PendingTask> {
        match domain {
            // 待收货：采购 PO（未收完）+ 生产工单（完工未入库）双来源（取消来料通知后统一入口）
            WorkCenterDomain::Arrival => {
                let mut tasks = Vec::new();

                // —— 采购待收货：PO status IN (Confirmed, PartiallyReceived) ——
                let po_svc = new_purchase_order_service(self.pool.clone());
                if let Ok(r) = po_svc
                    .list(ctx, db, PurchaseOrderQuery::default(), PageParams::new(1, FETCH_LIMIT))
                    .await
                {
                    let pos: Vec<_> = r
                        .items
                        .into_iter()
                        .filter(|o| matches!(
                            o.status,
                            PurchaseOrderStatus::Confirmed | PurchaseOrderStatus::PartiallyReceived
                        ))
                        .collect();
                    let supplier_ids: Vec<i64> = pos.iter().map(|o| o.supplier_id).collect();
                    let names = resolve_supplier_names(ctx, db, self.pool.clone(), &supplier_ids).await;
                    for o in pos {
                        tasks.push(PendingTask {
                            doc_id: o.id,
                            doc_number: o.doc_number,
                            domain,
                            source_kind: TaskSourceKind::PurchaseOrder,
                            counterparty: names
                                .get(&o.supplier_id)
                                .cloned()
                                .unwrap_or_else(|| format!("供应商 #{}", o.supplier_id)),
                            summary: "采购待收".into(),
                            expected_at: o.expected_delivery_date.map(midnight_utc),
                            urgency: o
                                .expected_delivery_date
                                .map_or(Urgency::Normal, |d| urgency_from_date(d, today)),
                        });
                    }
                }

                // —— 生产待入库：工单 status IN (Released, InProduction) 且 completed_qty > 已入库 ——
                let wo_svc = new_work_order_service(self.pool.clone());
                if let Ok(r) = wo_svc
                    .list(ctx, db, WorkOrderFilter::default(), 1, FETCH_LIMIT)
                    .await
                {
                    let wos: Vec<_> = r
                        .items
                        .into_iter()
                        .filter(|w| matches!(
                            w.status,
                            WorkOrderStatus::Released | WorkOrderStatus::InProduction
                        ))
                        .collect();
                    let product_ids: Vec<i64> = wos.iter().map(|w| w.product_id).collect();
                    let product_names =
                        resolve_product_names(ctx, db, self.pool.clone(), &product_ids).await;
                    let inv_svc = new_inventory_transaction_service(self.pool.clone());
                    for w in wos {
                        let received: Decimal = inv_svc
                            .find_by_source(ctx, db, "work_order", w.id)
                            .await
                            .unwrap_or_default()
                            .iter()
                            .map(|t| t.quantity)
                            .sum();
                        if w.completed_qty > received {
                            tasks.push(PendingTask {
                                doc_id: w.id,
                                doc_number: w.doc_number,
                                domain,
                                source_kind: TaskSourceKind::WorkOrder,
                                counterparty: product_names
                                    .get(&w.product_id)
                                    .cloned()
                                    .unwrap_or_else(|| format!("产品 #{}", w.product_id)),
                                summary: format!("待入库 {}", w.completed_qty - received),
                                expected_at: Some(midnight_utc(w.scheduled_end)),
                                urgency: urgency_from_date(w.scheduled_end, today),
                            });
                        }
                    }
                }
                tasks
            }
            // 待拣货：无到期日，用 created_at 判超时
            WorkCenterDomain::Pick => {
                let svc = new_pick_list_service(self.pool.clone());
                match svc
                    .list(
                        ctx,
                        db,
                        PickListQuery { status: Some(PickListStatus::Draft), ..Default::default() },
                        PageParams::new(1, FETCH_LIMIT),
                    )
                    .await
                {
                    Ok(r) => r
                        .items
                        .into_iter()
                        .map(|p| PendingTask {
                            doc_id: p.id,
                            doc_number: p.doc_number,
                            domain,
                            source_kind: TaskSourceKind::PurchaseOrder,
                            counterparty: "拣货作业".into(),
                            summary: "待拣货".into(),
                            expected_at: Some(p.created_at),
                            urgency: urgency_from_age(p.created_at, now),
                        })
                        .collect(),
                    Err(e) => {
                        tracing::warn!(domain = "pick", error = %e, "list_pending fetch failed");
                        vec![]
                    }
                }
            }
            // 待发货：Confirmed + Picking 两状态合并
            WorkCenterDomain::Outbound => {
                let svc = new_shipping_request_service(self.pool.clone());
                let mut tasks = Vec::new();
                for status in [ShippingStatus::Confirmed, ShippingStatus::Picking] {
                    if let Ok(r) = svc
                        .list(
                            ctx,
                            db,
                            ShippingQuery { status: Some(status), ..Default::default() },
                            PageParams::new(1, FETCH_LIMIT),
                        )
                        .await
                    {
                        let customer_ids: Vec<i64> = r.items.iter().map(|s| s.customer_id).collect();
                        let names =
                            resolve_customer_names(ctx, db, self.pool.clone(), &customer_ids).await;
                        for s in r.items {
                            let exp = s.expected_ship_date;
                            tasks.push(PendingTask {
                                doc_id: s.id,
                                doc_number: s.doc_number,
                                domain,
                                source_kind: TaskSourceKind::PurchaseOrder,
                                counterparty: names
                                    .get(&s.customer_id)
                                    .cloned()
                                    .unwrap_or_else(|| format!("客户 #{}", s.customer_id)),
                                summary: "发货".into(),
                                expected_at: exp.map(midnight_utc),
                                urgency: exp.map_or(Urgency::Normal, |d| urgency_from_date(d, today)),
                            });
                        }
                    }
                }
                tasks
            }
            // 待领料：Confirmed + PartiallyIssued
            WorkCenterDomain::Requisition => {
                let svc = new_material_requisition_service(self.pool.clone());
                let mut tasks = Vec::new();
                for status in [RequisitionStatus::Confirmed, RequisitionStatus::PartiallyIssued] {
                    if let Ok(r) = svc
                        .list(
                            ctx,
                            db,
                            RequisitionFilter { status: Some(status), ..Default::default() },
                            1,
                            FETCH_LIMIT,
                        )
                        .await
                    {
                        let wo_ids: Vec<i64> = r.items.iter().map(|m| m.work_order_id).collect();
                        let won =
                            resolve_work_order_numbers(ctx, db, self.pool.clone(), &wo_ids).await;
                        for m in r.items {
                            tasks.push(PendingTask {
                                doc_id: m.id,
                                doc_number: m.doc_number,
                                domain,
                                source_kind: TaskSourceKind::PurchaseOrder,
                                counterparty: won
                                    .get(&m.work_order_id)
                                    .cloned()
                                    .unwrap_or_else(|| format!("工单 #{}", m.work_order_id)),
                                summary: "领料".into(),
                                expected_at: Some(midnight_utc(m.requisition_date)),
                                urgency: urgency_from_date(m.requisition_date, today),
                            });
                        }
                    }
                }
                tasks
            }
            // 待调拨：Draft + InTransit
            WorkCenterDomain::Transfer => {
                let svc = new_transfer_service(self.pool.clone());
                let mut tasks = Vec::new();
                for status in [TransferStatus::Draft, TransferStatus::InTransit] {
                    if let Ok(r) = svc
                        .list(
                            ctx,
                            db,
                            TransferFilter { status: Some(status), ..Default::default() },
                            1,
                            FETCH_LIMIT,
                        )
                        .await
                    {
                        let wh_ids: Vec<i64> = r
                            .items
                            .iter()
                            .flat_map(|t| [t.from_warehouse_id, t.to_warehouse_id])
                            .collect();
                        let whn =
                            resolve_warehouse_names(ctx, db, self.pool.clone(), &wh_ids).await;
                        for t in r.items {
                            tasks.push(PendingTask {
                                doc_id: t.id,
                                doc_number: t.doc_number,
                                domain,
                                source_kind: TaskSourceKind::PurchaseOrder,
                                counterparty: format!(
                                    "{}→{}",
                                    whn.get(&t.from_warehouse_id)
                                        .cloned()
                                        .unwrap_or_else(|| format!("#{}", t.from_warehouse_id)),
                                    whn.get(&t.to_warehouse_id)
                                        .cloned()
                                        .unwrap_or_else(|| format!("#{}", t.to_warehouse_id)),
                                ),
                                summary: t
                                    .item_count
                                    .map(|c| format!("{c} 项"))
                                    .unwrap_or_else(|| "调拨".into()),
                                expected_at: Some(midnight_utc(t.transfer_date)),
                                urgency: urgency_from_date(t.transfer_date, today),
                            });
                        }
                    }
                }
                tasks
            }
            // 待盘点：Draft + Counting + PendingReview
            WorkCenterDomain::CycleCount => {
                let svc = new_cycle_count_service(self.pool.clone());
                let mut tasks = Vec::new();
                for status in [
                    CycleCountStatus::Draft,
                    CycleCountStatus::Counting,
                    CycleCountStatus::PendingReview,
                ] {
                    if let Ok(r) = svc
                        .list(
                            ctx,
                            db,
                            CycleCountFilter { status: Some(status), ..Default::default() },
                            1,
                            FETCH_LIMIT,
                        )
                        .await
                    {
                        let wh_ids: Vec<i64> = r.items.iter().map(|c| c.warehouse_id).collect();
                        let whn =
                            resolve_warehouse_names(ctx, db, self.pool.clone(), &wh_ids).await;
                        for c in r.items {
                            tasks.push(PendingTask {
                                doc_id: c.id,
                                doc_number: c.doc_number,
                                domain,
                                source_kind: TaskSourceKind::PurchaseOrder,
                                counterparty: whn
                                    .get(&c.warehouse_id)
                                    .cloned()
                                    .unwrap_or_else(|| format!("仓 #{}", c.warehouse_id)),
                                summary: c
                                    .item_count
                                    .map(|x| format!("{x} 项"))
                                    .unwrap_or_else(|| "盘点".into()),
                                expected_at: Some(midnight_utc(c.count_date)),
                                urgency: urgency_from_date(c.count_date, today),
                            });
                        }
                    }
                }
                tasks
            }
        }
    }
}

/// 单域待办计数：查询失败（如依赖表未建）不连累整个 summary，log warn 后记 0。
/// 作业中心是聚合看板，容错保证部分域可用时仍展示其余域。
async fn cnt<T>(domain: &'static str, f: impl std::future::Future<Output = Result<PaginatedResult<T>>>) -> u64 {
    match f.await {
        Ok(r) => r.total,
        Err(e) => {
            tracing::warn!(domain, error = %e, "work_center count failed, recorded as 0");
            0
        }
    }
}

/// `NaiveDate` → 当天 0 点 UTC（`PendingTask.expected_at` 统一用 `DateTime<Utc>`）
fn midnight_utc(d: NaiveDate) -> DateTime<Utc> {
    Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
}

/// 按到期日判紧急度（用于有 expected_date 的环节）
fn urgency_from_date(expected: NaiveDate, today: NaiveDate) -> Urgency {
    if expected < today {
        Urgency::Overdue
    } else if expected <= today + Days::new(SOON_DAYS) {
        Urgency::Soon
    } else {
        Urgency::Normal
    }
}

/// 按创建时长判超时（拣货等无到期日的环节）
fn urgency_from_age(created_at: DateTime<Utc>, now: DateTime<Utc>) -> Urgency {
    if created_at < now - chrono::Duration::hours(PICK_TIMEOUT_HOURS) {
        Urgency::Overdue
    } else {
        Urgency::Normal
    }
}

// ── 跨域名称解析（id → 真实名/单号），拉一批 filter 避免 N+1 ──

/// 供应商 id → name
async fn resolve_supplier_names(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: PgPool,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    new_supplier_service(pool)
        .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 500))
        .await
        .map(|r| {
            r.items
                .into_iter()
                .filter(|s| ids.contains(&s.id))
                .map(|s| (s.id, s.name))
                .collect()
        })
        .unwrap_or_default()
}

/// 客户 id → name
async fn resolve_customer_names(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: PgPool,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    new_customer_service(pool)
        .list(ctx, db, CustomerQuery::default(), PageParams::new(1, 500))
        .await
        .map(|r| {
            r.items
                .into_iter()
                .filter(|c| ids.contains(&c.id))
                .map(|c| (c.id, c.name))
                .collect()
        })
        .unwrap_or_default()
}

/// 仓库 id → name
async fn resolve_warehouse_names(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: PgPool,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    new_warehouse_service(pool)
        .list(ctx, db, WarehouseFilter::default(), 1, 500)
        .await
        .map(|r| {
            r.items
                .into_iter()
                .filter(|w| ids.contains(&w.id))
                .map(|w| (w.id, w.name))
                .collect()
        })
        .unwrap_or_default()
}

/// 工单 id → doc_number（WorkOrder 无批量接口，逐个 find_by_id；pending 队列已 top N 截断）
async fn resolve_work_order_numbers(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: PgPool,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let svc = new_work_order_service(pool);
    let mut map = HashMap::new();
    for id in ids {
        if let Ok(wo) = svc.find_by_id(ctx, db, *id).await {
            map.insert(*id, wo.doc_number);
        }
    }
    map
}

/// 产品 id → pdt_name（工单待入库的 counterparty 用）
async fn resolve_product_names(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: PgPool,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    new_product_service(pool)
        .get_by_ids(ctx, db, ids.to_vec())
        .await
        .map(|r| r.into_iter().map(|p| (p.product_id, p.pdt_name)).collect())
        .unwrap_or_default()
}

fn urgency_rank(u: Urgency) -> u8 {
    match u {
        Urgency::Overdue => 2,
        Urgency::Soon => 1,
        Urgency::Normal => 0,
    }
}

#[async_trait]
impl WorkCenterService for WorkCenterServiceImpl {
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WorkCenterSummary> {
        let pool = self.pool.clone();

        // 待收货：采购 PO 未收完 + 生产工单完工未入库（双来源，复用 fetch_domain_tasks）
        let today = Utc::now().date_naive();
        let now = Utc::now();
        let arrivals_pending = self
            .fetch_domain_tasks(ctx, db, WorkCenterDomain::Arrival, today, now)
            .await
            .len() as u64;

        // 待拣货（Draft）—— pick_list（依赖 migration 071 的 pick_lists 表）
        let picks_pending = cnt("picks", new_pick_list_service(pool.clone()).list(
            ctx, db, PickListQuery { status: Some(PickListStatus::Draft), ..Default::default() }, PageParams::new(1, 1),
        )).await;

        // 待发货（Confirmed + Picking）—— outbound
        let out = new_shipping_request_service(pool.clone());
        let outbounds_pending = cnt("outbounds_confirmed", out.list(
            ctx, db, ShippingQuery { status: Some(ShippingStatus::Confirmed), ..Default::default() }, PageParams::new(1, 1),
        )).await
            + cnt("outbounds_picking", out.list(
                ctx, db, ShippingQuery { status: Some(ShippingStatus::Picking), ..Default::default() }, PageParams::new(1, 1),
            )).await;

        // 待领料（Confirmed + PartiallyIssued）—— material_requisition
        let req = new_material_requisition_service(pool.clone());
        let requisitions_pending = cnt("requisitions_confirmed", req.list(
            ctx, db, RequisitionFilter { status: Some(RequisitionStatus::Confirmed), ..Default::default() }, 1, 1,
        )).await
            + cnt("requisitions_partial", req.list(
                ctx, db, RequisitionFilter { status: Some(RequisitionStatus::PartiallyIssued), ..Default::default() }, 1, 1,
            )).await;

        // 待调拨（Draft + InTransit）—— transfer
        let trf = new_transfer_service(pool.clone());
        let transfers_pending = cnt("transfers_draft", trf.list(
            ctx, db, TransferFilter { status: Some(TransferStatus::Draft), ..Default::default() }, 1, 1,
        )).await
            + cnt("transfers_intransit", trf.list(
                ctx, db, TransferFilter { status: Some(TransferStatus::InTransit), ..Default::default() }, 1, 1,
            )).await;

        // 待盘点（Draft + Counting + PendingReview）—— cycle_count
        let cyc = new_cycle_count_service(pool.clone());
        let cycle_counts_pending = cnt("cycle_draft", cyc.list(
            ctx, db, CycleCountFilter { status: Some(CycleCountStatus::Draft), ..Default::default() }, 1, 1,
        )).await
            + cnt("cycle_counting", cyc.list(
                ctx, db, CycleCountFilter { status: Some(CycleCountStatus::Counting), ..Default::default() }, 1, 1,
            )).await
            + cnt("cycle_pending_review", cyc.list(
                ctx, db, CycleCountFilter { status: Some(CycleCountStatus::PendingReview), ..Default::default() }, 1, 1,
            )).await;

        Ok(WorkCenterSummary {
            arrivals_pending,
            picks_pending,
            outbounds_pending,
            requisitions_pending,
            transfers_pending,
            cycle_counts_pending,
        })
    }

    async fn list_pending(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>> {
        let today = Utc::now().date_naive();
        let now = Utc::now();
        let mut tasks = self.fetch_domain_tasks(ctx, db, domain, today, now).await;
        // 排序：紧急度高在前；同等级早到期在前；无到期日（None）排后
        tasks.sort_by(|a, b| {
            urgency_rank(b.urgency)
                .cmp(&urgency_rank(a.urgency))
                .then_with(|| match (a.expected_at, b.expected_at) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                })
        });
        let total = tasks.len() as u64;
        let start = (page.page as usize).saturating_sub(1) * page.page_size as usize;
        let items = tasks
            .into_iter()
            .skip(start)
            .take(page.page_size as usize)
            .collect();
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn urgent_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<UrgentSummary> {
        let today = Utc::now().date_naive();
        let now = Utc::now();
        let mut by_domain = HashMap::new();
        for domain in ALL_DOMAINS {
            let mut overdue = 0u64;
            let mut soon = 0u64;
            for t in self.fetch_domain_tasks(ctx, db, domain, today, now).await {
                match t.urgency {
                    Urgency::Overdue => overdue += 1,
                    Urgency::Soon => soon += 1,
                    Urgency::Normal => {}
                }
            }
            by_domain.insert(domain, (overdue, soon));
        }
        Ok(UrgentSummary { by_domain })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn urgency_overdue_when_past_today() {
        let today = d(2026, 6, 25);
        assert_eq!(urgency_from_date(d(2026, 6, 24), today), Urgency::Overdue);
        assert_eq!(urgency_from_date(d(2026, 6, 20), today), Urgency::Overdue);
    }

    #[test]
    fn urgency_soon_within_window() {
        let today = d(2026, 6, 25);
        // today 自身、+1、+2（=SOON_DAYS 上界）均为 Soon
        assert_eq!(urgency_from_date(today, today), Urgency::Soon);
        assert_eq!(urgency_from_date(d(2026, 6, 27), today), Urgency::Soon);
    }

    #[test]
    fn urgency_normal_beyond_window() {
        let today = d(2026, 6, 25);
        assert_eq!(urgency_from_date(d(2026, 6, 28), today), Urgency::Normal);
    }

    #[test]
    fn urgency_from_age_pick_timeout() {
        let now = Utc::now();
        // 创建超过阈值 → Overdue
        let stale = now - chrono::Duration::hours(PICK_TIMEOUT_HOURS + 1);
        assert_eq!(urgency_from_age(stale, now), Urgency::Overdue);
        // 刚创建 → Normal
        let fresh = now - chrono::Duration::hours(1);
        assert_eq!(urgency_from_age(fresh, now), Urgency::Normal);
    }
}
