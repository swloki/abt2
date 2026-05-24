use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait CostAccountingService: Send + Sync {
    /// 查询指定产品在某期间的成本汇总
    async fn get_product_cost(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        period: String,
    ) -> Result<ProductCostSummary, DomainError>;

    /// 查询指定工单的成本汇总
    async fn get_work_order_cost(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderCostSummary, DomainError>;

    /// 查询指定利润中心在时间范围内的汇总（分页）
    async fn get_profit_center_summary(
        &self,
        ctx: ServiceContext<'_>,
        profit_center_id: i64,
        from: String,
        to: String,
        page: PageParams,
    ) -> Result<PaginatedResult<ProfitCenterSummary>, DomainError>;

    /// 查询指定销售订单的毛利分析
    async fn get_margin_analysis(
        &self,
        ctx: ServiceContext<'_>,
        order_id: i64,
    ) -> Result<MarginAnalysis, DomainError>;
}
