use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::super::enums::WorkOrderStatus;
use super::model::*;
use super::repo::WorkOrderRepo;
use super::service::WorkOrderService;
use crate::master_data::bom::service::BomQueryService;
use crate::mes::production_batch::model::WorkOrderRouting;
use crate::mes::production_batch::repo::{ProductionBatchRepo, WorkOrderRoutingRepo};
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::{DocumentType, ReservationType};
use crate::shared::inventory_reservation::model::ReserveRequest;
use crate::shared::inventory_reservation::service::InventoryReservationService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::material_requisition::service::MaterialRequisitionService;

pub struct WorkOrderServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    inv_res: Arc<dyn InventoryReservationService>,
    material_req: Arc<dyn MaterialRequisitionService>,
    bom: Arc<dyn BomQueryService>,
}

impl WorkOrderServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        inv_res: Arc<dyn InventoryReservationService>,
        material_req: Arc<dyn MaterialRequisitionService>,
        bom: Arc<dyn BomQueryService>,
    ) -> Self {
        Self { pool, doc_seq, inv_res, material_req, bom }
    }
}

#[async_trait]
impl WorkOrderService for WorkOrderServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateWorkOrderReq,
    ) -> Result<i64> {
        let doc_number = self.doc_seq.next_number(ctx.reborrow(), DocumentType::WorkOrder)
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
    ) -> Result<WorkOrder> {
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
    ) -> Result<()> {
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

        // 3. 获取 BOM 叶子节点（组件）
        let bom_nodes = if let Some(bom_id) = work_order.bom_snapshot_id {
            self.bom.get_leaf_nodes(ctx.reborrow(), bom_id).await.unwrap_or_default()
        } else {
            vec![]
        };

        // 4. 从 BOM 节点创建 WorkOrderRouting
        if !bom_nodes.is_empty() {
            let routing_steps: Vec<WorkOrderRouting> = bom_nodes
                .iter()
                .enumerate()
                .map(|(i, node)| WorkOrderRouting {
                    id: 0,
                    work_order_id: id,
                    step_no: (i + 1) as i32,
                    process_name: node.product_code.clone().unwrap_or_default(),
                    work_center_id: None,
                    standard_time: None,
                    standard_cost: None,
                    unit_price: None,
                    allowed_loss_rate: Some(node.loss_rate),
                    planned_qty: node.quantity,
                    completed_qty: Decimal::ZERO,
                    defect_qty: Decimal::ZERO,
                    status: super::super::enums::RoutingStatus::Pending,
                    is_outsourced: false,
                    is_inspection_point: false,
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

        let batch_no = self.doc_seq.next_number(
            ctx.reborrow(),
            DocumentType::WorkOrder,
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
        let _ = self.inv_res.reserve(
            ctx.reborrow(),
            vec![ReserveRequest {
                product_id: work_order.product_id,
                warehouse_id: 0,
                reserved_qty: work_order.planned_qty,
                reservation_type: ReservationType::Hard,
                source_type: DocumentType::WorkOrder,
                source_id: id,
                source_line_id: None,
                priority: 0,
                expires_at: None,
            }],
        )
        .await;

        // 7. 创建领料单
        let _ = self.material_req.create_for_work_order(ctx.reborrow(), id).await;

        Ok(())
    }

    /// 关闭工单：Released -> Closed
    async fn close(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
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

        let _ = self.inv_res.cancel_by_source(ctx.reborrow(), DocumentType::WorkOrder, id).await;

        Ok(())
    }

    /// 取消工单：Draft/Planned/Released -> Cancelled
    async fn cancel(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
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

        let _ = self.inv_res.cancel_by_source(ctx.reborrow(), DocumentType::WorkOrder, id).await;
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
    ) -> Result<PaginatedResult<WorkOrder>> {
        WorkOrderRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
