use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use super::super::enums::InspectionResultType;
use super::model::*;

#[async_trait]
pub trait ProductionInspectionService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateInspectionReq,
    ) -> Result<i64>;
    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ProductionInspection>;
    async fn record_result(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        result: InspectionResultType,
    ) -> Result<()>;
}
