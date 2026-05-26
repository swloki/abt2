use crate::fms::cost_accounting::model::*;
use crate::fms::cost_accounting::repo::CostAccountingRepo;
use crate::fms::cost_accounting::service::CostAccountingService;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

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
        ctx: ServiceContext<'_>,
        product_id: i64,
        period: String,
    ) -> Result<ProductCostSummary, DomainError> {
        CostAccountingRepo::get_product_cost_by_period(ctx.executor, product_id, &period)
            .await
            
    }

    async fn get_work_order_cost(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderCostSummary, DomainError> {
        CostAccountingRepo::get_work_order_cost(ctx.executor, work_order_id)
            .await
            
    }

    async fn get_profit_center_summary(
        &self,
        ctx: ServiceContext<'_>,
        profit_center_id: i64,
        from: String,
        to: String,
        page: PageParams,
    ) -> Result<PaginatedResult<ProfitCenterSummary>, DomainError> {
        let (items, total) = CostAccountingRepo::get_profit_center_summary(
            ctx.executor,
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
        ctx: ServiceContext<'_>,
        order_id: i64,
    ) -> Result<MarginAnalysis, DomainError> {
        CostAccountingRepo::get_margin_analysis(ctx.executor, order_id)
            .await
            
    }
}
