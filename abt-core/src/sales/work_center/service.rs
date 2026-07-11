use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor, Result};

use super::model::{
    QuotationHubSummary, SalesOrderHubSummary, SalesReturnHubSummary, SalesWorkCenterSummary,
    SettlementHubSummary, SettlementReconType,
};

/// 销售作业中心聚合服务（只读视图，写操作复用 quotation/sales_order/sales_return/reconciliation 既有 Service）。
#[async_trait]
pub trait SalesWorkCenterService: Send + Sync {
    /// 聚合各业务分组待办计数（首页锚点条 + 各 card 用）。
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<SalesWorkCenterSummary>;

    /// 销售订单行展开聚合（订单 card row-detail）。
    /// 聚合发货进度（明细 shipped/open/returned）、来源链（报价单派生）、应收台账（ArApService 客户维度）。
    async fn get_order_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<SalesOrderHubSummary>;

    /// 报价单行展开聚合（报价 card row-detail：明细 + 可转单状态）。
    async fn get_quotation_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<QuotationHubSummary>;

    /// 销售退货行展开聚合（退货 card row-detail：来源 SO + 收货进度）。
    async fn get_return_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        return_id: i64,
    ) -> Result<SalesReturnHubSummary>;

    /// 对账收发行展开聚合（按对象类型分发：草稿/待发送对账单、待结算对账单 + 客户 AR 未清）。
    async fn get_settlement_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        recon_type: SettlementReconType,
        ref_id: i64,
    ) -> Result<SettlementHubSummary>;
}
