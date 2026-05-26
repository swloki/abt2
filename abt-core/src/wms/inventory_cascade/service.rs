use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, ServiceContext, Result};

#[async_trait]
pub trait InventoryCascadeService: Send + Sync {
    async fn cascade_inventory(&self, ctx: ServiceContext<'_>, query: CascadeInventoryQuery) -> Result<CascadeInventoryResult>;
}
