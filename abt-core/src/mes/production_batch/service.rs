use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use super::model::*;

#[async_trait]
pub trait ProductionBatchService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateBatchReq) -> Result<i64, DomainError>;
    async fn split_work_order(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
        splits: Vec<SplitReq>,
    ) -> Result<Vec<i64>, DomainError>;
    async fn find_by_id(&self, ctx: ServiceContext<'_>, id: i64) -> Result<ProductionBatch, DomainError>;
    async fn list_by_work_order(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<Vec<ProductionBatch>, DomainError>;
    async fn confirm_routing_step(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
        step_no: i32,
        req: StepConfirmationReq,
    ) -> Result<StepConfirmationResult, DomainError>;
    async fn advance_to_receipt(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
    ) -> Result<(), DomainError>;
    async fn suspend(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
        reason: String,
    ) -> Result<(), DomainError>;
    async fn resume(&self, ctx: ServiceContext<'_>, batch_id: i64) -> Result<(), DomainError>;
    async fn scrap(&self, ctx: ServiceContext<'_>, batch_id: i64, reason: String) -> Result<(), DomainError>;
}
