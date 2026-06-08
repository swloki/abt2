use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait CostAccountingService: Send + Sync {
    /// 查询指定产品在某期间的成本汇总
    async fn get_product_cost(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        period: String,
    ) -> Result<ProductCostSummary>;

    /// 查询指定工单的成本汇总
    async fn get_work_order_cost(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderCostSummary>;

    /// 查询指定利润中心在时间范围内的汇总（分页）
    async fn get_profit_center_summary(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        profit_center_id: i64,
        from: String,
        to: String,
        page: PageParams,
    ) -> Result<PaginatedResult<ProfitCenterSummary>>;

    /// 查询指定销售订单的毛利分析
    async fn get_margin_analysis(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<MarginAnalysis>;
    /// 查询指定期间所有产品的成本汇总列表
    async fn list_product_costs(
        &self, db: PgExecutor<'_>, period: &str,
    ) -> Result<Vec<ProductCostRow>>;

    /// 查询所有工单的成本汇总列表
    async fn list_work_order_costs(
        &self, db: PgExecutor<'_>,
    ) -> Result<Vec<WorkOrderCostRow>>;

    /// 查询所有利润中心 P&L
    async fn list_profit_center_pl(
        &self, db: PgExecutor<'_>, period: &str,
    ) -> Result<Vec<ProfitCenterPLRow>>;

    /// 查询所有销售订单的毛利数据
    async fn list_margin_analysis(
        &self, db: PgExecutor<'_>,
    ) -> Result<Vec<MarginRow>>;
}
