use async_trait::async_trait;
use sqlx::PgPool;

use super::model::FmsWorkCenterSummary;
use super::service::FmsWorkCenterService;
use crate::fms::adjustment::{new_adjustment_service, AdjustmentFilter, AdjustmentService};
use crate::fms::ar_ap::{
    new_ar_ap_service, ArApLedgerFilter, ArApService, LedgerSummary, SettlementFilter,
};
use crate::fms::enums::CounterpartyType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{DomainError, PageParams, PgExecutor, Result};

pub struct FmsWorkCenterServiceImpl {
    pool: PgPool,
}

impl FmsWorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 单状态计数：查询失败（如依赖表未建）不连累整个 summary，log warn 后记 0。
async fn cnt<T>(
    label: &'static str,
    f: impl std::future::Future<Output = Result<PaginatedResult<T>>>,
) -> u64 {
    match f.await {
        Ok(r) => r.total,
        Err(e) => {
            tracing::warn!(label, error = %e, "fms work_center count failed, recorded as 0");
            0
        }
    }
}

/// 台账汇总：查询失败 log warn 后记默认（全 0），不连累整页。
async fn amt(
    label: &'static str,
    f: impl std::future::Future<Output = Result<LedgerSummary>>,
) -> LedgerSummary {
    match f.await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                label,
                error = %e,
                "fms work_center ledger_summary failed, recorded as default"
            );
            LedgerSummary::default()
        }
    }
}

#[async_trait]
impl FmsWorkCenterService for FmsWorkCenterServiceImpl {
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<FmsWorkCenterSummary> {
        let pool = &self.pool;
        let one = PageParams::new(1, 1);
        let _ = db;

        // 7 个查询并发：2× ledger_summary（AR/AP 金额）+ 2× list_ledger outstanding（AR/AP 笔数）
        // + 2× list_adjustments（AR/AP 调整总数）+ 1× list_settlements。失败 best-effort 容错。
        let (
            ar_summary,
            ap_summary,
            ar_count,
            ap_count,
            ar_adjustment,
            ap_adjustment,
            settlement_total,
        ) = tokio::join!(
            amt("ar_summary", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_ar_ap_service(pool.clone())
                    .ledger_summary(ctx, &mut c, ArApLedgerFilter {
                        party_type: Some(CounterpartyType::Customer),
                        ..Default::default()
                    })
                    .await
            }),
            amt("ap_summary", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_ar_ap_service(pool.clone())
                    .ledger_summary(ctx, &mut c, ArApLedgerFilter {
                        party_type: Some(CounterpartyType::Supplier),
                        ..Default::default()
                    })
                    .await
            }),
            cnt("ar_count", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_ar_ap_service(pool.clone())
                    .list_ledger(ctx, &mut c, ArApLedgerFilter {
                        party_type: Some(CounterpartyType::Customer),
                        outstanding_only: true,
                        ..Default::default()
                    }, one.clone())
                    .await
            }),
            cnt("ap_count", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_ar_ap_service(pool.clone())
                    .list_ledger(ctx, &mut c, ArApLedgerFilter {
                        party_type: Some(CounterpartyType::Supplier),
                        outstanding_only: true,
                        ..Default::default()
                    }, one.clone())
                    .await
            }),
            cnt("ar_adjustment", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_adjustment_service(pool.clone())
                    .list_adjustments(ctx, &mut c, AdjustmentFilter {
                        party_type: Some(CounterpartyType::Customer),
                        ..Default::default()
                    }, one.clone())
                    .await
            }),
            cnt("ap_adjustment", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_adjustment_service(pool.clone())
                    .list_adjustments(ctx, &mut c, AdjustmentFilter {
                        party_type: Some(CounterpartyType::Supplier),
                        ..Default::default()
                    }, one.clone())
                    .await
            }),
            cnt("settlement_total", async {
                let mut c = pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
                new_ar_ap_service(pool.clone())
                    .list_settlements(ctx, &mut c, SettlementFilter::default(), one.clone())
                    .await
            }),
        );

        Ok(FmsWorkCenterSummary {
            ar_outstanding_amount: ar_summary.total_outstanding,
            ar_overdue_amount: ar_summary.total_overdue,
            ar_due_soon_amount: ar_summary.due_within_7d,
            ar_outstanding_count: ar_count,
            ap_outstanding_amount: ap_summary.total_outstanding,
            ap_overdue_amount: ap_summary.total_overdue,
            ap_due_soon_amount: ap_summary.due_within_7d,
            ap_outstanding_count: ap_count,
            ar_adjustment_total: ar_adjustment,
            ap_adjustment_total: ap_adjustment,
            settlement_total,
        })
    }
}
