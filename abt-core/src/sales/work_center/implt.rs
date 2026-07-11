use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::model::{
    QuotationHubSummary, ReconciliationAggregate, SalesOrderArSummary, SalesOrderHubSummary,
    SalesOrderProgress, SalesOrderSourceChain, SalesReturnHubSummary, SalesWorkCenterSummary,
    SettlementHubSummary, SettlementReconType,
};
use super::service::SalesWorkCenterService;
use crate::fms::ar_ap::{new_ar_ap_service, ArApLedgerFilter, ArApService};
use crate::fms::enums::CounterpartyType;
use crate::master_data::customer::{new_customer_service, CustomerService};
use crate::sales::quotation::model::{QuotationQuery, QuotationStatus};
use crate::sales::quotation::{new_quotation_service, QuotationService};
use crate::sales::reconciliation::model::{ReconciliationQuery, ReconciliationStatus};
use crate::sales::reconciliation::{new_reconciliation_service, ReconciliationService};
use crate::sales::sales_order::model::{SalesOrderItem, SalesOrderQuery, SalesOrderStatus};
use crate::sales::sales_order::{new_sales_order_service, SalesOrderService};
use crate::sales::sales_return::model::{ReturnQuery, ReturnStatus};
use crate::sales::sales_return::{new_sales_return_service, SalesReturnService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{DomainError, PageParams, PgExecutor, Result};

pub struct SalesWorkCenterServiceImpl {
    pool: PgPool,
}

impl SalesWorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 客户名（best-effort：缺失则空串，前端可兜底）。
    async fn customer_name(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        customer_id: i64,
    ) -> String {
        new_customer_service(self.pool.clone())
            .get(ctx, db, customer_id)
            .await
            .map(|c| c.name)
            .unwrap_or_default()
    }

    /// 客户 AR 台账摘要（经 ArApService::ledger_summary 客户维度；best-effort）。
    ///
    /// AR 在发货时由 `ShipmentShippedHandler` 立账（source_type=ShippingRequest，非销售订单），
    /// 故用客户维度而非订单维度近似。
    async fn ar_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        customer_id: i64,
    ) -> SalesOrderArSummary {
        let filter = ArApLedgerFilter {
            party_type: Some(CounterpartyType::Customer),
            party_id: Some(customer_id),
            ..Default::default()
        };
        let s = new_ar_ap_service(self.pool.clone())
            .ledger_summary(ctx, db, filter)
            .await
            .unwrap_or_default();
        SalesOrderArSummary {
            ar_amount: s.total_amount,
            outstanding: s.total_outstanding,
        }
    }

    /// 客户 AR 未清余额（对账 card 行展开用；best-effort）。
    async fn ar_outstanding(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        customer_id: i64,
    ) -> Decimal {
        self.ar_summary(ctx, db, customer_id).await.outstanding
    }
}

/// 聚合销售订单明细的发货进度（纯计算）。
fn order_progress(items: &[SalesOrderItem]) -> SalesOrderProgress {
    let ordered = items.iter().map(|i| i.quantity).sum::<Decimal>();
    let shipped = items.iter().map(|i| i.shipped_qty).sum::<Decimal>();
    let returned = items.iter().map(|i| i.returned_qty).sum::<Decimal>();
    let open = items.iter().map(|i| i.open_qty()).sum::<Decimal>();
    let shipped_pct = if ordered > Decimal::ZERO {
        (shipped / ordered * Decimal::from(100)).min(Decimal::from(100))
    } else {
        Decimal::ZERO
    };
    SalesOrderProgress {
        ordered_qty: ordered,
        shipped_qty: shipped,
        open_qty: open,
        returned_qty: returned,
        shipped_pct,
        item_count: items.len(),
    }
}

/// 销售退货状态文案（基于状态推导）。
fn return_status_hint(s: ReturnStatus) -> &'static str {
    match s {
        ReturnStatus::Draft => "草稿，确认后待收货",
        ReturnStatus::Confirmed => "已确认，待客户发货",
        ReturnStatus::Received => "已收货，待检验",
        ReturnStatus::Inspecting => "检验中，待完成",
        ReturnStatus::Completed => "已完成（冲减应收）",
        ReturnStatus::Cancelled => "已取消",
        ReturnStatus::Rejected => "已拒绝",
    }
}

/// 单状态计数：查询失败（如依赖表未建）不连累整个 summary，log warn 后记 0。
/// 作业中心是聚合看板，容错保证部分状态可用时仍展示其余（同采购 work_center）。
async fn cnt<T>(
    label: &'static str,
    f: impl std::future::Future<Output = Result<PaginatedResult<T>>>,
) -> u64 {
    match f.await {
        Ok(r) => r.total,
        Err(e) => {
            tracing::warn!(label, error = %e, "sales work_center count failed, recorded as 0");
            0
        }
    }
}

#[async_trait]
impl SalesWorkCenterService for SalesWorkCenterServiceImpl {
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<SalesWorkCenterSummary> {
        let pool = &self.pool;
        let one = PageParams::new(1, 1);

        // 18 个计数查询并发：各自从连接池获取连接（互不阻塞）。
        // acquire/查询失败由 cnt 容错记 0（best-effort）。
        let (
            quotation_draft, quotation_sent, quotation_accepted,
            order_draft, order_confirmed, order_ready, order_shipping, order_partial,
            return_pending, return_pending_receive, return_pending_inspect,
            recon_draft, recon_sent, recon_confirmed,
            total_quotations, total_orders, total_returns, total_recon,
        ) = tokio::join!(
            cnt("quo_draft", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_quotation_service(pool.clone()).list(ctx, &mut c, QuotationQuery { status: Some(QuotationStatus::Draft), ..Default::default() }, one.clone()).await
            }),
            cnt("quo_sent", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_quotation_service(pool.clone()).list(ctx, &mut c, QuotationQuery { status: Some(QuotationStatus::Sent), ..Default::default() }, one.clone()).await
            }),
            cnt("quo_accepted", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_quotation_service(pool.clone()).list(ctx, &mut c, QuotationQuery { status: Some(QuotationStatus::Accepted), ..Default::default() }, one.clone()).await
            }),
            cnt("order_draft", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_order_service(pool.clone()).list(ctx, &mut c, SalesOrderQuery { status: Some(SalesOrderStatus::Draft), ..Default::default() }, one.clone()).await
            }),
            cnt("order_confirmed", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_order_service(pool.clone()).list(ctx, &mut c, SalesOrderQuery { status: Some(SalesOrderStatus::Confirmed), ..Default::default() }, one.clone()).await
            }),
            cnt("order_ready", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_order_service(pool.clone()).list(ctx, &mut c, SalesOrderQuery { status: Some(SalesOrderStatus::ReadyToShip), ..Default::default() }, one.clone()).await
            }),
            cnt("order_shipping", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_order_service(pool.clone()).list(ctx, &mut c, SalesOrderQuery { status: Some(SalesOrderStatus::ShippingRequested), ..Default::default() }, one.clone()).await
            }),
            cnt("order_partial", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_order_service(pool.clone()).list(ctx, &mut c, SalesOrderQuery { status: Some(SalesOrderStatus::PartiallyShipped), ..Default::default() }, one.clone()).await
            }),
            cnt("return_draft", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_return_service(pool.clone()).list(ctx, &mut c, ReturnQuery { status: Some(ReturnStatus::Draft), ..Default::default() }, one.clone()).await
            }),
            cnt("return_confirmed", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_return_service(pool.clone()).list(ctx, &mut c, ReturnQuery { status: Some(ReturnStatus::Confirmed), ..Default::default() }, one.clone()).await
            }),
            cnt("return_received", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_return_service(pool.clone()).list(ctx, &mut c, ReturnQuery { status: Some(ReturnStatus::Received), ..Default::default() }, one.clone()).await
            }),
            cnt("recon_draft", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_reconciliation_service(pool.clone()).list(ctx, &mut c, ReconciliationQuery { status: Some(ReconciliationStatus::Draft), ..Default::default() }, one.clone()).await
            }),
            cnt("recon_sent", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_reconciliation_service(pool.clone()).list(ctx, &mut c, ReconciliationQuery { status: Some(ReconciliationStatus::Sent), ..Default::default() }, one.clone()).await
            }),
            cnt("recon_confirmed", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_reconciliation_service(pool.clone()).list(ctx, &mut c, ReconciliationQuery { status: Some(ReconciliationStatus::Confirmed), ..Default::default() }, one.clone()).await
            }),
            cnt("total_quotations", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_quotation_service(pool.clone()).list(ctx, &mut c, QuotationQuery::default(), one.clone()).await
            }),
            cnt("total_orders", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_order_service(pool.clone()).list(ctx, &mut c, SalesOrderQuery::default(), one.clone()).await
            }),
            cnt("total_returns", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_sales_return_service(pool.clone()).list(ctx, &mut c, ReturnQuery::default(), one.clone()).await
            }),
            cnt("total_recon", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_reconciliation_service(pool.clone()).list(ctx, &mut c, ReconciliationQuery::default(), one.clone()).await
            }),
        );

        // AR 汇总（客户维度，全量）：未清余额 + 逾期金额（best-effort）。
        let ar = new_ar_ap_service(pool.clone())
            .ledger_summary(
                ctx,
                db,
                ArApLedgerFilter {
                    party_type: Some(CounterpartyType::Customer),
                    ..Default::default()
                },
            )
            .await
            .unwrap_or_default();

        Ok(SalesWorkCenterSummary {
            quotation_draft,
            quotation_sent,
            quotation_accepted,
            order_draft,
            // Confirmed + ReadyToShip 合并为「待发货」
            order_pending_ship: order_confirmed + order_ready,
            order_shipping,
            order_partial,
            return_pending,
            return_pending_receive,
            return_pending_inspect,
            recon_draft,
            recon_sent,
            recon_confirmed,
            ar_outstanding_amount: ar.total_outstanding,
            ar_overdue_amount: ar.total_overdue,
            total_quotations,
            total_orders,
            total_returns,
            total_recon,
        })
    }

    async fn get_order_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<SalesOrderHubSummary> {
        let order_svc = new_sales_order_service(self.pool.clone());
        let order = order_svc.find_by_id(ctx, db, order_id).await?;

        let customer_name = self.customer_name(ctx, db, order.customer_id).await;
        let items = order_svc.list_items(ctx, db, order_id).await?;
        let progress = order_progress(&items);
        // 来源报价单链：第一阶段不反查（DocumentLink trait 无按 target 查），留空 best-effort。
        let source_chain = SalesOrderSourceChain::default();
        let ar_summary = self.ar_summary(ctx, db, order.customer_id).await;

        Ok(SalesOrderHubSummary {
            order,
            customer_name,
            progress,
            source_chain,
            ar_summary,
        })
    }

    async fn get_quotation_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<QuotationHubSummary> {
        let q_svc = new_quotation_service(self.pool.clone());
        let quotation = q_svc.find_by_id(ctx, db, quotation_id).await?;
        let customer_name = self.customer_name(ctx, db, quotation.customer_id).await;
        let item_count = q_svc
            .list_items(ctx, db, quotation_id)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        let can_convert_to_so = quotation.status == QuotationStatus::Accepted;
        Ok(QuotationHubSummary {
            total_amount: quotation.total_amount,
            quotation,
            customer_name,
            item_count,
            can_convert_to_so,
        })
    }

    async fn get_return_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        return_id: i64,
    ) -> Result<SalesReturnHubSummary> {
        let r_svc = new_sales_return_service(self.pool.clone());
        let so_svc = new_sales_order_service(self.pool.clone());
        let return_order = r_svc.find_by_id(ctx, db, return_id).await?;
        let customer_name = self.customer_name(ctx, db, return_order.customer_id).await;
        let source_so_doc = match so_svc.find_by_id(ctx, db, return_order.order_id).await {
            Ok(so) => so.doc_number,
            Err(_) => format!("#{}", return_order.order_id),
        };
        let items = r_svc.list_items(ctx, db, return_id).await.unwrap_or_default();
        let total_qty = items.iter().map(|i| i.returned_qty).sum::<Decimal>();
        let status_hint = return_status_hint(return_order.status).into();
        Ok(SalesReturnHubSummary {
            return_order,
            customer_name,
            source_so_doc,
            item_count: items.len(),
            total_qty,
            status_hint,
        })
    }

    async fn get_settlement_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        recon_type: SettlementReconType,
        ref_id: i64,
    ) -> Result<SettlementHubSummary> {
        let recon_svc = new_reconciliation_service(self.pool.clone());
        let recon = recon_svc.find_by_id(ctx, db, ref_id).await?;
        let customer_name = self.customer_name(ctx, db, recon.customer_id).await;
        let item_count = recon_svc
            .list_items(ctx, db, ref_id)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        let ar_outstanding = self.ar_outstanding(ctx, db, recon.customer_id).await;
        Ok(SettlementHubSummary {
            recon_type,
            customer_name,
            recon: ReconciliationAggregate {
                id: recon.id,
                status: recon.status,
                doc_number: recon.doc_number,
                period: recon.period,
                total_amount: recon.total_amount,
                confirmed_amount: recon.confirmed_amount,
                difference: recon.difference,
                item_count,
            },
            ar_outstanding,
        })
    }
}
