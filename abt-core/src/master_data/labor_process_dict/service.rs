use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait LaborProcessDictService: Send + Sync {
    async fn list(&self, ctx: &ServiceContext, db: PgExecutor<'_>, query: LaborProcessDictQuery, page: PageParams) -> Result<PaginatedResult<LaborProcessDict>>;
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<LaborProcessDict>;
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateLaborProcessDictReq) -> Result<i64>;
    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateLaborProcessDictReq) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
