use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::enums::PlanStatus;
use super::super::stubs::DocumentSequenceStub;
use super::model::*;
use super::repo::ProductionPlanRepo;
use super::service::ProductionPlanService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

pub struct ProductionPlanServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl ProductionPlanServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionPlanService for ProductionPlanServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreatePlanReq,
    ) -> Result<i64, DomainError> {
        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "PP-")
            .await
            .unwrap_or_else(|_| format!("PP{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let plan = ProductionPlanRepo::insert(
            &mut *ctx.executor,
            &req,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if !req.items.is_empty() {
            ProductionPlanRepo::insert_items(&mut *ctx.executor, plan.id, &req.items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(plan.id)
    }

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ProductionPlan, DomainError> {
        ProductionPlanRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionPlan"))
    }

    async fn confirm(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let plan = ProductionPlanRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionPlan"))?;

        if plan.status != PlanStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: plan.status.to_string(),
                to: PlanStatus::Confirmed.to_string(),
            });
        }

        ProductionPlanRepo::update_status(&mut *ctx.executor, id, PlanStatus::Confirmed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn release_to_work_orders(
        &self,
        _ctx: ServiceContext<'_>,
        plan_id: i64,
    ) -> Result<BatchReleaseResult, DomainError> {
        // TODO: 待 WorkOrder 模块实现后，将计划行下达为工单（循环依赖解决后再接入）
        Ok(BatchReleaseResult {
            plan_id,
            successful_work_orders: vec![],
            failed_items: vec![],
            total: 0,
        })
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: PlanFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ProductionPlan>, DomainError> {
        ProductionPlanRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
