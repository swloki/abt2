use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::enums::PlanStatus;
use super::model::*;
use super::repo::ProductionPlanRepo;
use super::service::ProductionPlanService;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::types::PgExecutor;
use crate::shared::enums::DocumentType;
use crate::mes::work_order::{new_work_order_service, model::CreateWorkOrderReq, service::WorkOrderService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

pub struct ProductionPlanServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl ProductionPlanServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionPlanService for ProductionPlanServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePlanReq,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ProductionPlan)
            .await
            .unwrap_or_else(|_| format!("PP{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let plan = ProductionPlanRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if !req.items.is_empty() {
            ProductionPlanRepo::insert_items(&mut *db, plan.id, &req.items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(plan.id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionPlan> {
        ProductionPlanRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionPlan"))
    }

    async fn confirm(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let plan = ProductionPlanRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionPlan"))?;

        if plan.status != PlanStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: plan.status.to_string(),
                to: PlanStatus::Confirmed.to_string(),
            });
        }

        ProductionPlanRepo::update_status(&mut *db, id, PlanStatus::Confirmed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn release_to_work_orders(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<BatchReleaseResult> {
        let items = ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let mut successful = Vec::new();
        let mut failed = Vec::new();

        let work_order_svc = new_work_order_service(self.pool.clone());

        for item in &items {
            let scheduled_start = chrono::Local::now().date_naive();
            let scheduled_end = scheduled_start + chrono::Duration::days(7);

            match work_order_svc.create(
                ctx, db,
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
                    if let Ok(wo) = work_order_svc.find_by_id(ctx, db, wo_id).await {
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
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: PlanFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ProductionPlan>> {
        ProductionPlanRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
