use super::model::*;
use super::service::InventoryCascadeService;
use crate::shared::types::{DomainError, ServiceContext};

#[derive(Default)]
pub struct InventoryCascadeServiceImpl;

impl InventoryCascadeServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl InventoryCascadeService for InventoryCascadeServiceImpl {
    async fn cascade_inventory(&self, _ctx: ServiceContext<'_>, _query: CascadeInventoryQuery) -> Result<CascadeInventoryResult, DomainError> {
        // TODO: 实现递归 BOM 展开查询
        Err(DomainError::validation("InventoryCascade: 功能尚未实现"))
    }
}
