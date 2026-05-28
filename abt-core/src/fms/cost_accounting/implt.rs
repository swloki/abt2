use crate::fms::cost_accounting::model::*;
use crate::fms::cost_accounting::repo::CostAccountingRepo;
use crate::fms::cost_accounting::service::CostAccountingService;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

/// 成本核算服务实现 — 所有方法均为只读查询，无共享服务依赖
#[derive(Default)]
pub struct CostAccountingServiceImpl;

impl CostAccountingServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl CostAccountingService for CostAccountingServiceImpl {
    async fn get_product_cost(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        period: String,
    ) -> Result<ProductCostSummary> {
        CostAccountingRepo::get_product_cost_by_period(db, product_id, &period)
            .await
            
    }

    async fn get_work_order_cost(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderCostSummary> {
        CostAccountingRepo::get_work_order_cost(db, work_order_id)
            .await
            
    }

    async fn get_profit_center_summary(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        profit_center_id: i64,
        from: String,
        to: String,
        page: PageParams,
    ) -> Result<PaginatedResult<ProfitCenterSummary>> {
        let (items, total) = CostAccountingRepo::get_profit_center_summary(
            db,
            profit_center_id,
            &from,
            &to,
            page.page_size,
            page.offset(),
        )
        .await
        ?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn get_margin_analysis(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<MarginAnalysis> {
        CostAccountingRepo::get_margin_analysis(db, order_id)
            .await
            
    }
}
