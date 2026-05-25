use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::enums::PlanStatus;
use super::model::*;
use super::repo::ProductionPlanRepo;
use super::service::ProductionPlanService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::DocumentType;
use crate::mes::work_order::model::CreateWorkOrderReq;
use crate::mes::work_order::service::WorkOrderService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

pub struct ProductionPlanServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    work_order: Arc<dyn WorkOrderService>,
}

impl ProductionPlanServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        work_order: Arc<dyn WorkOrderService>,
    ) -> Self {
        Self { pool, doc_seq, work_order }
    }
}

#[async_trait]
impl ProductionPlanService for ProductionPlanServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreatePlanReq,
    ) -> Result<i64, DomainError> {
        let doc_number = self.doc_seq.next_number(ctx.reborrow(), DocumentType::ProductionPlan)
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
        mut ctx: ServiceContext<'_>,
        plan_id: i64,
    ) -> Result<BatchReleaseResult, DomainError> {
        let items = ProductionPlanRepo::get_items_by_plan_id(&mut *ctx.executor, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for item in &items {
            let scheduled_start = chrono::Local::now().date_naive();
            let scheduled_end = scheduled_start + chrono::Duration::days(7);

            match self.work_order.create(
                ctx.reborrow(),
                CreateWorkOrderReq {
                    plan_item_id: Some(item.id),
                    product_id: item.product_id,
                    bom_snapshot_id: None,
                    routing_id: None,
                    planned_qty: item.planned_qty,
                    scheduled_start,
                    scheduled_end,
                    work_center_id: None,
                    sales_order_id: None,
                    remark: None,
                },
            ).await {
                Ok(wo_id) => {
                    if let Ok(wo) = self.work_order.find_by_id(ctx.reborrow(), wo_id).await {
                        successful.push(wo);
                    }
                }
                Err(e) => failed.push(BatchFailure {
                    index: item.id as i32,
                    error: e,
                }),
            }
        }

        let total = items.len() as i32;
        Ok(BatchReleaseResult { plan_id, successful_work_orders: successful, failed_items: failed, total })
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
