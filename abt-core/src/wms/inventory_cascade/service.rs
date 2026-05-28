use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,ServiceContext, Result};

#[async_trait]
pub trait InventoryCascadeService: Send + Sync {
    async fn cascade_inventory(&self, ctx: &ServiceContext, db: PgExecutor<'_>, query: CascadeInventoryQuery) -> Result<CascadeInventoryResult>;
}
