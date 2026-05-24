use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{IssueMaterialReq, MaterialRequisition, RequisitionFilter};
use super::repo::MaterialRequisitionRepo;
use super::service::MaterialRequisitionService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::RequisitionStatus;
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::service::InventoryTransactionService;
use crate::wms::stubs::{
    DocumentLinkStub, DocumentSequenceStub, InventoryReservationStub, ReservationType,
    WorkOrderStub,
};

pub struct MaterialRequisitionServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    inventory_transaction_svc: Arc<dyn InventoryTransactionService>,
}

impl MaterialRequisitionServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        inventory_transaction_svc: Arc<dyn InventoryTransactionService>,
    ) -> Self {
        Self { pool, inventory_transaction_svc }
    }
}

#[async_trait]
impl MaterialRequisitionService for MaterialRequisitionServiceImpl {
    async fn create_for_work_order(
        &self,
        mut ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<i64, DomainError> {
        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "MR-")
            .await
            .unwrap_or_else(|_| format!("MR{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let requisition_date = chrono::Local::now().date_naive();

        let wo_info = WorkOrderStub::get_info(ctx.reborrow(), work_order_id).await?;
        let warehouse_id = wo_info.warehouse_id;

        let requisition = MaterialRequisitionRepo::insert(
            &mut *ctx.executor,
            &doc_number,
            work_order_id,
            requisition_date,
            warehouse_id,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let _ = DocumentLinkStub::link(
            ctx.reborrow(),
            "material_requisition",
            requisition.id,
            "work_order",
            work_order_id,
        )
        .await;

        Ok(requisition.id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<MaterialRequisition, DomainError> {
        MaterialRequisitionRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("MaterialRequisition"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: RequisitionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<MaterialRequisition>, DomainError> {
        MaterialRequisitionRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn confirm(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let requisition = MaterialRequisitionRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("MaterialRequisition"))?;

        if requisition.status != RequisitionStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", requisition.status),
                to: "Confirmed".to_string(),
            });
        }

        let affected = MaterialRequisitionRepo::update_status(
            &mut *ctx.executor,
            id,
            RequisitionStatus::Confirmed,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found("MaterialRequisition"));
        }

        Ok(())
    }

    /// 发料：Confirmed → Issued
    /// 设计：issue -> InvRes.fulfill(Hard) + InventoryTransaction.record(MaterialIssue)
    async fn issue(
        &self,
        mut ctx: ServiceContext<'_>,
        req: IssueMaterialReq,
    ) -> Result<(), DomainError> {
        let requisition = MaterialRequisitionRepo::get_by_id(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("MaterialRequisition"))?;

        if requisition.status != RequisitionStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", requisition.status),
                to: "Issued".to_string(),
            });
        }

        // 获取所有明细（用于查找 product_id）
        let existing_items = MaterialRequisitionRepo::get_items(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        for item in &req.items {
            let found = existing_items.iter().find(|i| i.id == item.item_id);
            if found.is_none() {
                return Err(DomainError::not_found(format!(
                    "MaterialReqItem {}",
                    item.item_id
                )));
            }
            let found_item = found.unwrap();
            let variance_qty = item.issued_qty - found_item.requested_qty;

            MaterialRequisitionRepo::update_item_issued(
                &mut *ctx.executor,
                item.item_id,
                item.issued_qty,
                variance_qty,
                item.bin_id,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            // issue -> InvRes.fulfill(Hard)
            let _ = InventoryReservationStub::fulfill(
                ctx.reborrow(),
                found_item.product_id,
                requisition.warehouse_id,
                item.issued_qty,
                ReservationType::Hard,
            )
            .await;

            // issue -> InventoryTransaction.record(MaterialIssue)
            // 领料为出库，quantity 为负值
            let _ = self.inventory_transaction_svc.record(
                ctx.reborrow(),
                RecordTransactionReq {
                    doc_number: None,
                    transaction_type: crate::wms::enums::TransactionType::MaterialIssue,
                    product_id: found_item.product_id,
                    warehouse_id: requisition.warehouse_id,
                    zone_id: None,
                    bin_id: item.bin_id,
                    batch_no: None,
                    quantity: -item.issued_qty,
                    unit_cost: None,
                    source_type: "material_requisition".to_string(),
                    source_id: req.id,
                    remark: None,
                },
            )
            .await;
        }

        let affected = MaterialRequisitionRepo::update_status(
            &mut *ctx.executor,
            req.id,
            RequisitionStatus::Issued,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found("MaterialRequisition"));
        }

        Ok(())
    }

    async fn cancel(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let requisition = MaterialRequisitionRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("MaterialRequisition"))?;

        if requisition.status != RequisitionStatus::Draft
            && requisition.status != RequisitionStatus::Confirmed
        {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", requisition.status),
                to: "Cancelled".to_string(),
            });
        }

        let affected =
            MaterialRequisitionRepo::soft_delete(&mut *ctx.executor, id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found("MaterialRequisition"));
        }

        Ok(())
    }
}
