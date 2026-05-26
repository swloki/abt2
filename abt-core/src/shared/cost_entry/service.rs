use async_trait::async_trait;

use crate::shared::enums::CostEntityType;
use crate::shared::types::batch::BatchResult;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{CostEntry, EntryRequest};

#[async_trait]
pub trait CostEntryService: Send + Sync {
    /// Atomic 模式 — 双层记账必须完整，事务中批量 INSERT
    async fn create_entries(
        &self,
        ctx: ServiceContext<'_>,
        entries: Vec<EntryRequest>,
    ) -> Result<BatchResult>;

    /// 分页查询某实体的成本分录
    async fn find_by_entity(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: CostEntityType,
        entity_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<CostEntry>>;
}
