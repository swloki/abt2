use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait BomLaborProcessService: Send + Sync {
    async fn list(&self, ctx: &ServiceContext, db: PgExecutor<'_>, query: BomLaborProcessQuery, page: PageParams) -> Result<PaginatedResult<BomLaborProcess>>;
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateBomLaborProcessReq) -> Result<i64>;
    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateBomLaborProcessReq) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
