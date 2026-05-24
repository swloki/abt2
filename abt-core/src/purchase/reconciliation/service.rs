use async_trait::async_trait;

use super::model::PurchaseReconciliation;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

#[async_trait]
pub trait PurchaseReconciliationService: Send + Sync {
    async fn create(
        ctx: ServiceContext<'_>,
        supplier_id: i64,
        period: String,
    ) -> Result<i64, DomainError>;

    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseReconciliation, DomainError>;

    async fn confirm(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
