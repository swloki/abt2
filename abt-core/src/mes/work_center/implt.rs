use async_trait::async_trait;
use sqlx::PgPool;

use super::model::MesWorkCenterSummary;
use super::service::MesWorkCenterService;
use crate::mes::enums::WorkOrderStatus;
use crate::mes::work_order::{new_work_order_service, WorkOrderFilter, WorkOrderService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PgExecutor, Result};

pub struct MesWorkCenterServiceImpl {
    pool: PgPool,
}

impl MesWorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 单状态计数：查询失败（如依赖表未建）不连累整个 summary，log warn 后记 0。
/// 作业中心是聚合看板，容错保证部分状态可用时仍展示其余。
async fn cnt<T>(label: &'static str, f: impl std::future::Future<Output = Result<PaginatedResult<T>>>) -> u64 {
    match f.await {
        Ok(r) => r.total,
        Err(e) => {
            tracing::warn!(label, error = %e, "mes work_center count failed, recorded as 0");
            0
        }
    }
}

#[async_trait]
impl MesWorkCenterService for MesWorkCenterServiceImpl {
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<MesWorkCenterSummary> {
        let svc = new_work_order_service(self.pool.clone());

        // 待下达：Draft + Planned
        let draft = cnt("wo_draft", svc.list(
            ctx, db,
            WorkOrderFilter { status: Some(WorkOrderStatus::Draft), ..Default::default() },
            1, 1,
        )).await;
        let planned = cnt("wo_planned", svc.list(
            ctx, db,
            WorkOrderFilter { status: Some(WorkOrderStatus::Planned), ..Default::default() },
            1, 1,
        )).await;

        // 生产中：Released + InProduction
        let released = cnt("wo_released", svc.list(
            ctx, db,
            WorkOrderFilter { status: Some(WorkOrderStatus::Released), ..Default::default() },
            1, 1,
        )).await;
        let inprod = cnt("wo_inprod", svc.list(
            ctx, db,
            WorkOrderFilter { status: Some(WorkOrderStatus::InProduction), ..Default::default() },
            1, 1,
        )).await;

        Ok(MesWorkCenterSummary {
            pending_release: draft + planned,
            in_production: released + inprod,
        })
    }
}
