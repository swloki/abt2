use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait BomQueryService: Send + Sync {
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, bom_id: i64) -> Result<Bom>;
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: BomQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Bom>>;
    async fn get_leaf_nodes(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<Vec<BomNode>>;
    async fn get_snapshots(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        version: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BomSnapshot>>;
    async fn exists_name(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        name: &str,
        caller_id: Option<i64>,
    ) -> Result<bool>;
}

#[async_trait]
pub trait BomCommandService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateBomReq) -> Result<i64>;
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateBomReq,
        expected_version: i32,
    ) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn publish(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<i64>;
    async fn unpublish(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn save_as(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_id: i64,
        new_name: String,
    ) -> Result<i64>;
    async fn substitute_product(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: SubstituteReq,
    ) -> Result<SubstitutionResult>;
    async fn validate_cycle(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<()>;
}

#[async_trait]
pub trait BomNodeService: Send + Sync {
    async fn add_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node: NewBomNode,
    ) -> Result<i64>;
    async fn update_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
        req: UpdateBomNodeReq,
        expected_version: i32,
    ) -> Result<()>;
    async fn delete_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
    ) -> Result<i64>;
    async fn move_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
        new_parent_id: i64,
        before_sibling_id: Option<i64>,
    ) -> Result<()>;
}

#[async_trait]
pub trait BomCostService: Send + Sync {
    async fn get_cost_report(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        as_of_date: Option<DateTime<Utc>>,
    ) -> Result<BomCostReport>;

    async fn get_labor_cost_report(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<BomLaborCostReport>;
}

#[async_trait]
pub trait BomCategoryService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateBomCategoryReq,
    ) -> Result<i64>;
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateBomCategoryReq,
    ) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: BomCategoryQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<BomCategory>>;
}
