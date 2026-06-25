use async_trait::async_trait;
use sqlx::PgPool;

use super::model::PurchaseWorkCenterSummary;
use super::service::PurchaseWorkCenterService;
use crate::purchase::demand_handler::{new_purchase_demand_service, MaterialAggQuery, PurchaseDemandService};
use crate::purchase::enums::{
    MiscRequestStatus, PaymentStatus, PurchaseOrderStatus, PurchaseReconStatus, PurchaseReturnStatus,
};
use crate::purchase::misc_request::model::MiscRequestQuery;
use crate::purchase::misc_request::{new_misc_request_service, MiscellaneousRequestService};
use crate::purchase::order::model::PurchaseOrderQuery;
use crate::purchase::order::{new_purchase_order_service, PurchaseOrderService};
use crate::purchase::payment::model::PaymentRequestQuery;
use crate::purchase::payment::{new_payment_request_service, PaymentRequestService};
use crate::purchase::reconciliation::model::PurchaseReconciliationQuery;
use crate::purchase::reconciliation::{new_purchase_reconciliation_service, PurchaseReconciliationService};
use crate::purchase::return_order::model::PurchaseReturnQuery;
use crate::purchase::return_order::{new_purchase_return_service, PurchaseReturnService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PageParams, PgExecutor, Result};

/// 扫描待收货订单以判定逾期/临期的取样条数（首页近似，避免全表扫描）。
const RECEIVING_SCAN_SIZE: u32 = 500;
/// 临期窗口（天）。
const SOON_WINDOW_DAYS: i64 = 7;

pub struct PurchaseWorkCenterServiceImpl {
    pool: PgPool,
}

impl PurchaseWorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 单状态计数：查询失败（如依赖表未建）不连累整个 summary，log warn 后记 0。
/// 作业中心是聚合看板，容错保证部分状态可用时仍展示其余（同 MES work_center）。
async fn cnt<T>(label: &'static str, f: impl std::future::Future<Output = Result<PaginatedResult<T>>>) -> u64 {
    match f.await {
        Ok(r) => r.total,
        Err(e) => {
            tracing::warn!(label, error = %e, "purchase work_center count failed, recorded as 0");
            0
        }
    }
}

#[async_trait]
impl PurchaseWorkCenterService for PurchaseWorkCenterServiceImpl {
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<PurchaseWorkCenterSummary> {
        let demand_svc = new_purchase_demand_service(self.pool.clone());
        let misc_svc = new_misc_request_service(self.pool.clone());
        let po_svc = new_purchase_order_service(self.pool.clone());
        let recon_svc = new_purchase_reconciliation_service(self.pool.clone());
        let pay_svc = new_payment_request_service(self.pool.clone());
        let ret_svc = new_purchase_return_service(self.pool.clone());

        let one = PageParams::new(1, 1);

        let pending_demand = cnt(
            "demand",
            demand_svc.list_material_aggregated(
                ctx,
                db,
                MaterialAggQuery::default(),
                one.clone(),
            ),
        )
        .await;

        let pending_misc = cnt(
            "misc",
            misc_svc.list(
                ctx,
                db,
                MiscRequestQuery {
                    status: Some(MiscRequestStatus::Draft),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let po_pending_approval = cnt(
            "po_approval",
            po_svc.list(
                ctx,
                db,
                PurchaseOrderQuery {
                    status: Some(PurchaseOrderStatus::PendingApproval),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let po_pending_receive = cnt(
            "po_confirmed",
            po_svc.list(
                ctx,
                db,
                PurchaseOrderQuery {
                    status: Some(PurchaseOrderStatus::Confirmed),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let po_partial = cnt(
            "po_partial",
            po_svc.list(
                ctx,
                db,
                PurchaseOrderQuery {
                    status: Some(PurchaseOrderStatus::PartiallyReceived),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let recon_draft = cnt(
            "recon_draft",
            recon_svc.list(
                ctx,
                db,
                PurchaseReconciliationQuery {
                    status: Some(PurchaseReconStatus::Draft),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let payment_pending_approval = cnt(
            "payment_draft",
            pay_svc.list(
                ctx,
                db,
                PaymentRequestQuery {
                    status: Some(PaymentStatus::Draft),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let return_pending_ship = cnt(
            "return_confirmed",
            ret_svc.list(
                ctx,
                db,
                PurchaseReturnQuery {
                    status: Some(PurchaseReturnStatus::Confirmed),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let return_shipped = cnt(
            "return_shipped",
            ret_svc.list(
                ctx,
                db,
                PurchaseReturnQuery {
                    status: Some(PurchaseReturnStatus::Shipped),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        // 逾期 / 临期：扫描待收货（Confirmed + PartiallyReceived）订单的期望交货日。
        // 近似统计（首页前 RECEIVING_SCAN_SIZE 条），查询失败按 0 容错。
        let today = chrono::Utc::now().date_naive();
        let soon_limit = today
            .checked_add_days(chrono::Days::new(SOON_WINDOW_DAYS as u64))
            .unwrap_or(today);
        let scan = PageParams::new(1, RECEIVING_SCAN_SIZE);
        let mut overdue_count = 0u64;
        let mut soon_count = 0u64;
        for st in [PurchaseOrderStatus::Confirmed, PurchaseOrderStatus::PartiallyReceived] {
            let items = po_svc
                .list(
                    ctx,
                    db,
                    PurchaseOrderQuery {
                        status: Some(st),
                        ..Default::default()
                    },
                    scan.clone(),
                )
                .await
                .map(|r| r.items)
                .unwrap_or_default();
            for o in items {
                if let Some(d) = o.expected_delivery_date {
                    if d < today {
                        overdue_count += 1;
                    } else if d <= soon_limit {
                        soon_count += 1;
                    }
                }
            }
        }

        Ok(PurchaseWorkCenterSummary {
            pending_demand,
            pending_misc,
            po_pending_approval,
            po_pending_receive,
            po_partial,
            recon_draft,
            payment_pending_approval,
            return_pending_ship,
            return_shipped,
            overdue_count,
            soon_count,
        })
    }
}
