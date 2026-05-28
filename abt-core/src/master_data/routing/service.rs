use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait RoutingService: Send + Sync {
    async fn list(&self, ctx: &ServiceContext, db: PgExecutor<'_>, query: RoutingQuery, page: PageParams) -> Result<PaginatedResult<Routing>>;
    async fn get_detail(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<RoutingDetail>;
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateRoutingReq) -> Result<i64>;
    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateRoutingReq) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn find_matching_routing(&self, ctx: &ServiceContext, db: PgExecutor<'_>, process_codes: Vec<String>) -> Result<Option<RoutingDetail>>;
    async fn set_bom_routing(&self, ctx: &ServiceContext, db: PgExecutor<'_>, product_code: String, routing_id: i64) -> Result<()>;
    async fn get_bom_routing(&self, ctx: &ServiceContext, db: PgExecutor<'_>, product_code: String) -> Result<Option<RoutingDetail>>;
    async fn list_boms_by_routing(&self, ctx: &ServiceContext, db: PgExecutor<'_>, routing_id: i64) -> Result<Vec<BomRouting>>;
}
