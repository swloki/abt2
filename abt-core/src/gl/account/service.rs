use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor, PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait GlAccountService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateGlAccountReq,
    ) -> Result<i64>;

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdateGlAccountReq,
    ) -> Result<()>;

    async fn get(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<GlAccount>;

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: GlAccountFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<GlAccount>>;

    /// 树形结构（parent_id 层级），用于前端科目树
    async fn get_tree(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<GlAccountNode>>;
}

#[derive(Debug, Clone)]
pub struct GlAccountNode {
    pub account: GlAccount,
    pub children: Vec<GlAccountNode>,
}
