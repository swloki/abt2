use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait RoutingService: Send + Sync {
    async fn list(&self, ctx: ServiceContext<'_>, query: RoutingQuery, page: PageParams) -> Result<PaginatedResult<Routing>, DomainError>;
    async fn get_detail(&self, ctx: ServiceContext<'_>, id: i64) -> Result<RoutingDetail, DomainError>;
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateRoutingReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateRoutingReq) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn find_matching_routing(&self, ctx: ServiceContext<'_>, process_codes: Vec<String>) -> Result<Option<RoutingDetail>, DomainError>;
    async fn set_bom_routing(&self, ctx: ServiceContext<'_>, product_code: String, routing_id: i64) -> Result<(), DomainError>;
    async fn get_bom_routing(&self, ctx: ServiceContext<'_>, product_code: String) -> Result<Option<RoutingDetail>, DomainError>;
    async fn list_boms_by_routing(&self, ctx: ServiceContext<'_>, routing_id: i64) -> Result<Vec<BomRouting>, DomainError>;
}
