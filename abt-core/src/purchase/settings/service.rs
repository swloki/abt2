use async_trait::async_trait;

use super::model::{PurchaseSettings, UpdatePurchaseSettingsRequest};
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

#[async_trait]
pub trait PurchaseSettingsService: Send + Sync {
    async fn get(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<PurchaseSettings>;

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpdatePurchaseSettingsRequest,
    ) -> Result<()>;
}
