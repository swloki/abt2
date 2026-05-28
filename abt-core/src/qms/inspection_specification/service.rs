use async_trait::async_trait;

use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};
use super::model::*;
use crate::qms::enums::InspectionType;

#[async_trait]
pub trait InspectionSpecificationService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateInspectionSpecificationReq,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<InspectionSpecification>;

    async fn find_by_product_and_type(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        inspection_type: InspectionType,
    ) -> Result<Option<InspectionSpecification>>;

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateInspectionSpecificationReq,
    ) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: InspectionSpecFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<InspectionSpecification>>;
}
