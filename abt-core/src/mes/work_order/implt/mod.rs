use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::super::enums::WorkOrderStatus;
use super::model::*;
use super::repo::WorkOrderRepo;
use super::service::WorkOrderService;
use crate::mes::production_batch::model::WorkOrderRouting;
use crate::mes::production_batch::repo::{ProductionBatchRepo, WorkOrderRoutingRepo};
use crate::mes::stubs::{
    BomServiceStub, DocumentSequenceStub, InventoryReservationStub,
    WmsMaterialRequisitionStub, ReservationType,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

pub struct WorkOrderServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl WorkOrderServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkOrderService for WorkOrderServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateWorkOrderReq,
    ) -> Result<i64, DomainError> {
        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "WO-")
            .await
            .unwrap_or_else(|_| format!("WO{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let work_order = WorkOrderRepo::insert(
            &mut *ctx.executor,
            &doc_number,
            &req,
            WorkOrderStatus::Draft,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(work_order.id)
    }

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<WorkOrder, DomainError> {
        WorkOrderRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))
    }

    /// 下达工单：Draft/Planned -> Released
    /// - 获取 BOM 快照 → 创建 WorkOrderRouting（工单级）
    /// - 创建至少 1 个 ProductionBatch
    /// - 库存 HARD 预留
    /// - 创建领料单
    async fn release(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<(), DomainError> {
        // 1. 验证工单存在且状态允许下达
        let work_order = WorkOrderRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Draft
            && work_order.status != WorkOrderStatus::Planned
        {
            return Err(DomainError::InvalidStateTransition {
                from: work_order.status.to_string(),
                to: WorkOrderStatus::Released.to_string(),
            });
        }

        // 2. 乐观锁更新状态
        let updated =
            WorkOrderRepo::update_status_with_version(
                &mut *ctx.executor,
                id,
                WorkOrderStatus::Released,
                expected_version,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        // 3. 获取 BOM 快照（工序 + 组件）
        let bom_snapshot = BomServiceStub::get_bom_snapshot(ctx.reborrow(), work_order.product_id)
            .await
            .unwrap_or_else(|_| crate::mes::stubs::BomSnapshot {
                routing_steps: vec![],
                components: vec![],
            });

        // 4. 从 BOM 快照创建 WorkOrderRouting（工单级，非批次级）
        if !bom_snapshot.routing_steps.is_empty() {
            let routing_steps: Vec<WorkOrderRouting> = bom_snapshot
                .routing_steps
                .iter()
                .map(|step| WorkOrderRouting {
                    id: 0,
                    work_order_id: id,
                    step_no: step.step_no,
                    process_name: step.process_name.clone(),
                    work_center_id: step.work_center_id,
                    standard_time: step.standard_time,
                    standard_cost: step.standard_cost,
                    unit_price: step.unit_price,
                    allowed_loss_rate: step.allowed_loss_rate,
                    planned_qty: step.planned_qty,
                    completed_qty: Decimal::ZERO,
                    defect_qty: Decimal::ZERO,
                    status: super::super::enums::RoutingStatus::Pending,
                    is_outsourced: step.is_outsourced,
                    is_inspection_point: step.is_inspection_point,
                })
                .collect();

            WorkOrderRoutingRepo::insert_for_work_order(&mut *ctx.executor, &routing_steps)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 5. 创建至少 1 个 ProductionBatch
        let batch_req = crate::mes::production_batch::model::CreateBatchReq {
            work_order_id: id,
            product_id: work_order.product_id,
            batch_qty: work_order.planned_qty,
            team_id: None,
        };

        let batch_no = DocumentSequenceStub::next_number(
            ctx.reborrow(),
            &format!("{}/", work_order.doc_number),
        )
        .await
        .unwrap_or_else(|_| format!("{}-01", work_order.doc_number));

        let card_sn = format!("SN-{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f"));

        ProductionBatchRepo::insert(
            &mut *ctx.executor,
            &batch_req,
            &batch_no,
            &card_sn,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 6. 库存 HARD 预留（planned_qty）
        // Note: warehouse_id 不在 WorkOrder 上，预留通过 stub 占位
        let _ = InventoryReservationStub::reserve(
            ctx.reborrow(),
            work_order.product_id,
            0, // warehouse_id 由领料单决定
            work_order.planned_qty,
            ReservationType::Hard,
        )
        .await;

        // 7. 创建领料单
        let _ = WmsMaterialRequisitionStub::create_for_work_order(
            ctx.reborrow(),
            id,
            work_order.product_id,
            0, // warehouse_id 由领料单决定
            work_order.planned_qty,
        )
        .await;

        Ok(())
    }

    /// 关闭工单：Released -> Closed
    /// 前置条件：所有生产批次已完成
    async fn close(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<(), DomainError> {
        let work_order = WorkOrderRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Released {
            return Err(DomainError::InvalidStateTransition {
                from: work_order.status.to_string(),
                to: WorkOrderStatus::Closed.to_string(),
            });
        }

        // 验证所有批次已完成
        let batches = ProductionBatchRepo::list_by_work_order(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let has_incomplete = batches.iter().any(|b| {
            b.status != super::super::enums::BatchStatus::Completed
                && b.status != super::super::enums::BatchStatus::Cancelled
        });

        if has_incomplete {
            return Err(DomainError::BusinessRule(
                "All production batches must be completed before closing the work order"
                    .to_string(),
            ));
        }

        let updated =
            WorkOrderRepo::update_status_with_version(
                &mut *ctx.executor,
                id,
                WorkOrderStatus::Closed,
                expected_version,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        let _ = InventoryReservationStub::release(ctx.reborrow(), "work_order", id).await;

        Ok(())
    }

    /// 取消工单：Draft/Planned/Released -> Cancelled
    async fn cancel(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<(), DomainError> {
        let work_order = WorkOrderRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Draft
            && work_order.status != WorkOrderStatus::Planned
            && work_order.status != WorkOrderStatus::Released
        {
            return Err(DomainError::InvalidStateTransition {
                from: work_order.status.to_string(),
                to: WorkOrderStatus::Cancelled.to_string(),
            });
        }

        let updated =
            WorkOrderRepo::update_status_with_version(
                &mut *ctx.executor,
                id,
                WorkOrderStatus::Cancelled,
                expected_version,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        let _ = InventoryReservationStub::release(ctx.reborrow(), "work_order", id).await;
        let _ = WorkOrderRepo::soft_delete(&mut *ctx.executor, id).await;
        let _ = WorkOrderRepo::soft_delete_batches(&mut *ctx.executor, id).await;

        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: WorkOrderFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<WorkOrder>, DomainError> {
        WorkOrderRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
