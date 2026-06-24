use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::WorkCenterSummary;
use super::service::WorkCenterService;
use crate::shared::types::pagination::PageParams;
use crate::shared::types::{PgExecutor, Result, ServiceContext};
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
use crate::wms::pick_list::{model::PickListQuery, new_pick_list_service, service::PickListService};
use crate::wms::transfer::{model::TransferFilter, new_transfer_service, service::TransferService};

pub struct WorkCenterServiceImpl {
    pool: PgPool,
}

impl WorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkCenterService for WorkCenterServiceImpl {
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WorkCenterSummary> {
        let pool = self.pool.clone();

        // 待收货（Draft）/ 待质检（Inspecting）—— arrival_notice
        let arrival = new_arrival_notice_service(pool.clone());
        let arrivals_pending = arrival
            .list(ctx, db, ArrivalNoticeFilter { status: Some(ArrivalStatus::Draft), ..Default::default() }, 1, 1)
            .await?
            .total;
        let inspections_pending = arrival
            .list(ctx, db, ArrivalNoticeFilter { status: Some(ArrivalStatus::Inspecting), ..Default::default() }, 1, 1)
            .await?
            .total;

        // 待拣货（Draft）—— pick_list
        let picks_pending = new_pick_list_service(pool.clone())
            .list(ctx, db, PickListQuery { status: Some(crate::wms::pick_list::model::PickListStatus::Draft), ..Default::default() }, PageParams::new(1, 1))
            .await?
            .total;

        // 待发货（Confirmed + Picking）—— outbound
        let out = new_shipping_request_service(pool.clone());
        let outbounds_pending = out
            .list(ctx, db, ShippingQuery { status: Some(ShippingStatus::Confirmed), ..Default::default() }, PageParams::new(1, 1))
            .await?
            .total
            + out
                .list(ctx, db, ShippingQuery { status: Some(ShippingStatus::Picking), ..Default::default() }, PageParams::new(1, 1))
                .await?
                .total;

        // 待领料（Confirmed + PartiallyIssued）—— material_requisition
        let req = new_material_requisition_service(pool.clone());
        let requisitions_pending = req
            .list(ctx, db, RequisitionFilter { status: Some(RequisitionStatus::Confirmed), ..Default::default() }, 1, 1)
            .await?
            .total
            + req
                .list(ctx, db, RequisitionFilter { status: Some(RequisitionStatus::PartiallyIssued), ..Default::default() }, 1, 1)
                .await?
                .total;

        // 待调拨（Draft + InTransit）—— transfer
        let trf = new_transfer_service(pool.clone());
        let transfers_pending = trf
            .list(ctx, db, TransferFilter { status: Some(TransferStatus::Draft), ..Default::default() }, 1, 1)
            .await?
            .total
            + trf
                .list(ctx, db, TransferFilter { status: Some(TransferStatus::InTransit), ..Default::default() }, 1, 1)
                .await?
                .total;

        // 待盘点（Draft + Counting + PendingReview）—— cycle_count
        let cyc = new_cycle_count_service(pool.clone());
        let cycle_counts_pending = cyc
            .list(ctx, db, CycleCountFilter { status: Some(CycleCountStatus::Draft), ..Default::default() }, 1, 1)
            .await?
            .total
            + cyc
                .list(ctx, db, CycleCountFilter { status: Some(CycleCountStatus::Counting), ..Default::default() }, 1, 1)
                .await?
                .total
            + cyc
                .list(ctx, db, CycleCountFilter { status: Some(CycleCountStatus::PendingReview), ..Default::default() }, 1, 1)
                .await?
                .total;

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
