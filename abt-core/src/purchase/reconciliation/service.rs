use async_trait::async_trait;

use super::model::{PurchaseReconItem, PurchaseReconPreviewItem, PurchaseReconciliation, PurchaseReconciliationQuery};
use crate::shared::types::PageParams;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

#[async_trait]
pub trait PurchaseReconciliationService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        supplier_id: i64,
        period: String,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PurchaseReconciliation>;

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: PurchaseReconciliationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseReconciliation>>;

    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        reconciliation_id: i64,
    ) -> Result<Vec<PurchaseReconItem>>;

    /// 预览某供应商某期间内「未对账已收货」的订单明细（只读，供创建页展示）。
    /// period 格式非法时返回空 Vec（宽松降级，不抛错）；严格校验留给 create。
    async fn preview(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
        period: String,
    ) -> Result<Vec<PurchaseReconPreviewItem>>;

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;
}
