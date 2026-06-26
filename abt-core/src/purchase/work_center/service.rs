use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor, Result};

use super::model::{
    PoHubSummary, PurchaseWorkCenterSummary, ReturnHubSummary, SettlementHubSummary,
    SettlementReconType, ThreeWayMatchSummary,
};

/// 采购作业中心聚合服务（只读视图，写操作复用各域既有 Service）。
#[async_trait]
pub trait PurchaseWorkCenterService: Send + Sync {
    /// 聚合各业务分组待办计数（首页锚点条 + 各 card 用）。
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<PurchaseWorkCenterSummary>;

    /// 采购订单行展开聚合（订单 card row-detail）。
    /// 聚合收货进度（明细）、来源链（DocumentLink PO→SO）、应付台账（ArApService）。
    async fn get_po_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<PoHubSummary>;

    /// 三单匹配校验查询（只读，复用 PaymentRequestServiceImpl::approve 的校验口径）。
    /// 给定付款申请 id，返回 PO/入库/发票 三项匹配状态 + 差异说明。
    async fn check_three_way_match(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        payment_id: i64,
    ) -> Result<ThreeWayMatchSummary>;

    /// 对账付款行展开聚合（按对象类型分发：草稿对账单 / 待审批付款）。
    async fn get_settlement_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        recon_type: SettlementReconType,
        ref_id: i64,
    ) -> Result<SettlementHubSummary>;

    /// 采购退货行展开聚合（退货 card row-detail）。
    async fn get_return_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        return_id: i64,
    ) -> Result<ReturnHubSummary>;
}
