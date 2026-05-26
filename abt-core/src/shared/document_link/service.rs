use async_trait::async_trait;

use crate::shared::enums::DocumentType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{DocumentLink, LinkRequest};

#[async_trait]
pub trait DocumentLinkService: Send + Sync {
    /// Atomic 模式 — 在事务中批量创建单据关联，全部成功或全部回滚
    async fn create_links(
        &self,
        ctx: ServiceContext<'_>,
        requests: Vec<LinkRequest>,
    ) -> Result<crate::shared::types::batch::BatchResult>;

    /// 双向分页查询：source→target 和 target→source
    async fn find_linked(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<DocumentLink>>;
}
