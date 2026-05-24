use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait BomQueryService: Send + Sync {
    async fn get(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<Bom, DomainError>;
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: BomQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Bom>, DomainError>;
    async fn get_leaf_nodes(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
    ) -> Result<Vec<BomNode>, DomainError>;
    async fn get_snapshots(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        version: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BomSnapshot>, DomainError>;
    async fn exists_name(
        &self,
        ctx: ServiceContext<'_>,
        name: &str,
        caller_id: Option<i64>,
    ) -> Result<bool, DomainError>;
}

#[async_trait]
pub trait BomCommandService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateBomReq) -> Result<i64, DomainError>;
    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateBomReq,
        expected_version: i32,
    ) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn publish(&self, ctx: ServiceContext<'_>, id: i64) -> Result<BomSnapshot, DomainError>;
    async fn unpublish(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn save_as(
        &self,
        ctx: ServiceContext<'_>,
        source_id: i64,
        new_name: String,
    ) -> Result<i64, DomainError>;
    async fn substitute_product(
        &self,
        ctx: ServiceContext<'_>,
        req: SubstituteReq,
    ) -> Result<SubstitutionResult, DomainError>;
    async fn validate_cycle(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
    ) -> Result<(), DomainError>;
}

#[async_trait]
pub trait BomNodeService: Send + Sync {
    async fn add_node(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        node: NewBomNode,
    ) -> Result<i64, DomainError>;
    async fn update_node(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        node_id: i64,
        req: UpdateBomNodeReq,
        expected_version: i32,
    ) -> Result<(), DomainError>;
    async fn delete_node(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        node_id: i64,
    ) -> Result<i64, DomainError>;
    async fn move_node(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        node_id: i64,
        new_parent_id: i64,
        before_sibling_id: Option<i64>,
    ) -> Result<(), DomainError>;
}

#[async_trait]
pub trait BomCostService: Send + Sync {
    async fn get_cost_report(
        &self,
        ctx: ServiceContext<'_>,
        bom_id: i64,
        as_of_date: Option<DateTime<Utc>>,
    ) -> Result<BomCostReport, DomainError>;
}

#[async_trait]
pub trait BomCategoryService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateBomCategoryReq,
    ) -> Result<i64, DomainError>;
    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateBomCategoryReq,
    ) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: BomCategoryQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<BomCategory>, DomainError>;
}
