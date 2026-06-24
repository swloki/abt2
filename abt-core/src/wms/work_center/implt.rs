use async_trait::async_trait;
use chrono::{DateTime, Days, NaiveDate, TimeZone, Utc};
use sqlx::postgres::PgPool;

use super::model::{PendingTask, Urgency, UrgentSummary, WorkCenterDomain, WorkCenterSummary};
use super::service::WorkCenterService;
use crate::shared::types::pagination::PageParams;
use crate::shared::types::{PaginatedResult, PgExecutor, Result, ServiceContext};
use crate::wms::arrival_notice::{
    model::ArrivalNoticeFilter, new_arrival_notice_service, service::ArrivalNoticeService,
};
use crate::wms::cycle_count::{
    model::CycleCountFilter, new_cycle_count_service, service::CycleCountService,
};
use crate::wms::enums::{ArrivalStatus, CycleCountStatus, RequisitionStatus, TransferStatus};
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

/// 临期阈值：today + N 天内到期视为 `Soon`。MVP 硬编码，后续进 wms/settings。
const SOON_DAYS: u64 = 2;
/// 拣货超时阈值：创建超过 N 小时视为 `Overdue`（拣货无到期日，用创建时长判超时）。
const PICK_TIMEOUT_HOURS: i64 = 4;
/// 单域拉取上限（与 `PageParams::page_size` clamp 上限对齐）。MVP：pending 超过此值的尾部不展示。
const FETCH_LIMIT: u32 = 200;

const ALL_DOMAINS: [WorkCenterDomain; 7] = [
    WorkCenterDomain::Arrival,
    WorkCenterDomain::Inspection,
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
            // 待收货（Draft）/ 待质检（Inspecting）共用 arrival_notice，仅状态不同
            WorkCenterDomain::Arrival | WorkCenterDomain::Inspection => {
                let status = if domain == WorkCenterDomain::Arrival {
                    ArrivalStatus::Draft
                } else {
                    ArrivalStatus::Inspecting
                };
                let svc = new_arrival_notice_service(self.pool.clone());
                match svc
                    .list(
                        ctx,
                        db,
                        ArrivalNoticeFilter { status: Some(status), ..Default::default() },
                        1,
                        FETCH_LIMIT,
                    )
                    .await
                {
                    Ok(r) => r
                        .items
                        .into_iter()
                        .map(|a| PendingTask {
                            doc_id: a.id,
                            doc_number: a.doc_number,
                            domain,
                            counterparty: format!("供应商 #{}", a.supplier_id),
                            summary: if domain == WorkCenterDomain::Arrival {
                                "来料收货".into()
                            } else {
                                "待质检".into()
                            },
                            expected_at: Some(midnight_utc(a.arrival_date)),
                            urgency: urgency_from_date(a.arrival_date, today),
                        })
                        .collect(),
                    Err(e) => {
                        tracing::warn!(domain = "arrival", error = %e, "list_pending fetch failed");
                        vec![]
                    }
                }
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
                        for s in r.items {
                            let exp = s.expected_ship_date;
                            tasks.push(PendingTask {
                                doc_id: s.id,
                                doc_number: s.doc_number,
                                domain,
                                counterparty: format!("客户 #{}", s.customer_id),
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
                        for m in r.items {
                            tasks.push(PendingTask {
                                doc_id: m.id,
                                doc_number: m.doc_number,
                                domain,
                                counterparty: format!("工单 #{}", m.work_order_id),
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
                        for t in r.items {
                            tasks.push(PendingTask {
                                doc_id: t.id,
                                doc_number: t.doc_number,
                                domain,
                                counterparty: format!("仓 {}→{}", t.from_warehouse_id, t.to_warehouse_id),
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
                        for c in r.items {
                            tasks.push(PendingTask {
                                doc_id: c.id,
                                doc_number: c.doc_number,
                                domain,
                                counterparty: format!("仓 #{}", c.warehouse_id),
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

        // 待收货（Draft）/ 待质检（Inspecting）—— arrival_notice
        let arrival = new_arrival_notice_service(pool.clone());
        let arrivals_pending = cnt("arrivals", arrival.list(
            ctx, db, ArrivalNoticeFilter { status: Some(ArrivalStatus::Draft), ..Default::default() }, 1, 1,
        )).await;
        let inspections_pending = cnt("inspections", arrival.list(
            ctx, db, ArrivalNoticeFilter { status: Some(ArrivalStatus::Inspecting), ..Default::default() }, 1, 1,
        )).await;

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
            inspections_pending,
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
        let mut overdue = 0u64;
        let mut soon = 0u64;
        for domain in ALL_DOMAINS {
            let tasks = self.fetch_domain_tasks(ctx, db, domain, today, now).await;
            for t in tasks {
                match t.urgency {
                    Urgency::Overdue => overdue += 1,
                    Urgency::Soon => soon += 1,
                    Urgency::Normal => {}
                }
            }
        }
        Ok(UrgentSummary { overdue_count: overdue, soon_count: soon })
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
