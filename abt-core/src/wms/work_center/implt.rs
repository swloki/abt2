use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::WorkCenterSummary;
use super::service::WorkCenterService;
use crate::shared::types::pagination::PageParams;
use crate::shared::types::{PaginatedResult, PgExecutor, Result, ServiceContext};
use crate::wms::arrival_notice::{
    model::ArrivalNoticeFilter, new_arrival_notice_service, service::ArrivalNoticeService,
};
use crate::wms::cycle_count::{model::CycleCountFilter, new_cycle_count_service, service::CycleCountService};
use crate::wms::enums::{ArrivalStatus, CycleCountStatus, RequisitionStatus, TransferStatus};
use crate::wms::material_requisition::{
    model::RequisitionFilter, new_material_requisition_service, service::MaterialRequisitionService,
};
use crate::wms::outbound::{
    model::{ShippingQuery, ShippingStatus}, new_shipping_request_service, service::ShippingRequestService,
};
use crate::wms::pick_list::{model::{PickListQuery, PickListStatus}, new_pick_list_service, service::PickListService};
use crate::wms::transfer::{model::TransferFilter, new_transfer_service, service::TransferService};

pub struct WorkCenterServiceImpl {
    pool: PgPool,
}

impl WorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
}
