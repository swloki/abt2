use async_trait::async_trait;

use super::model::*;
use crate::fms::enums::CounterpartyType;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait ArApService: Send + Sync {
    // ---- 台账查询 ----

    /// 查询应收应付台账（分页，含往来方名称和科目信息）
    async fn list_ledger(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ArApLedgerFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ArApLedgerRow>>;

    /// 查询台账明细（产品行项目级，供「导出明细表」用，不分页，遵循同一 filter）
    async fn list_ledger_details(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ArApLedgerFilter,
    ) -> Result<Vec<ArApLedgerDetailRow>>;

    /// 台账汇总（按 filter 聚合：总额/未清/逾期/7天内到期，逾期基准=due_date）
    async fn ledger_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ArApLedgerFilter,
    ) -> Result<LedgerSummary>;

    /// 获取单个往来方当前余额
    async fn get_party_balance(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<PartyBalance>;

    /// 批量获取往来方余额（用于列表页）
    async fn batch_party_balances(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_ids: &[i64],
    ) -> Result<Vec<PartyBalance>>;

    // ---- 核销 ----

    /// 执行核销：将付款与发票匹配
    /// 支持一笔付款核销多张发票、部分核销
    async fn settle(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: SettleReq,
    ) -> Result<SettleResult>;

    /// 取消核销（反核销）
    async fn unsettle(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        settlement_id: i64,
    ) -> Result<()>;

    /// 查询核销记录列表
    async fn list_settlements(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: SettlementFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ArApSettlement>>;

    // ---- 账龄分析 ----

    /// 应收账龄分析（按客户）
    async fn ar_aging(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: AgingReq,
    ) -> Result<Vec<AgingRow>>;

    /// 应付账龄分析（按供应商）
    async fn ap_aging(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: AgingReq,
    ) -> Result<Vec<AgingRow>>;

    // ---- 详情（drawer） ----

    /// 获取台账详情：台账行（含 party_name/upstream）+ 产品行项目清单
    async fn get_ledger_detail(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<(ArApLedgerRow, Vec<LedgerDetailItem>)>>;

    // ---- 未清项查询（用于核销选择器） ----

    /// 查询某往来方的未清发票
    async fn list_open_invoices(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<Vec<OpenInvoice>>;

    /// 查询某往来方未分配的收款/付款
    async fn list_unapplied_payments(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<Vec<UnappliedPayment>>;
}
