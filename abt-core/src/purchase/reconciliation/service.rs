use async_trait::async_trait;

use super::model::PurchaseReconciliation;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

#[async_trait]
pub trait PurchaseReconciliationService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        supplier_id: i64,
        period: String,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseReconciliation>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;
}
