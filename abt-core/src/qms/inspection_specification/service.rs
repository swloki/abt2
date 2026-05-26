use async_trait::async_trait;

use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};
use super::model::*;
use crate::qms::enums::InspectionType;

#[async_trait]
pub trait InspectionSpecificationService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateInspectionSpecificationReq,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<InspectionSpecification>;

    async fn find_by_product_and_type(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        inspection_type: InspectionType,
    ) -> Result<Option<InspectionSpecification>>;

    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateInspectionSpecificationReq,
    ) -> Result<()>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: InspectionSpecFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<InspectionSpecification>>;
}
