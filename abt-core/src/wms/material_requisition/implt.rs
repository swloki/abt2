use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{IssueMaterialReq, MaterialRequisition, RequisitionFilter};
use super::repo::MaterialRequisitionRepo;
use super::service::MaterialRequisitionService;
use crate::mes::work_order::{new_work_order_service, service::WorkOrderService};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::types::PgExecutor;
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::{DocumentType, LinkType};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::RequisitionStatus;
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::{new_inventory_transaction_service, service::InventoryTransactionService};

pub struct MaterialRequisitionServiceImpl {
    pool: PgPool,
}

impl MaterialRequisitionServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MaterialRequisitionService for MaterialRequisitionServiceImpl {
    async fn create_for_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::MaterialRequisition)
            .await
            .unwrap_or_else(|_| format!("MR{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let requisition_date = chrono::Local::now().date_naive();

        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, db, work_order_id).await?;
        let warehouse_id = wo.work_center_id.unwrap_or(0);

        let requisition = MaterialRequisitionRepo::insert(
            &mut *db,
            &doc_number,
            work_order_id,
            requisition_date,
            warehouse_id,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        new_document_link_service(self.pool.clone())
        .create_links(
            ctx, db,
            vec![LinkRequest {
                source_type: DocumentType::MaterialRequisition,
                source_id: requisition.id,
                target_type: DocumentType::WorkOrder,
                target_id: work_order_id,
                link_type: LinkType::Fulfills,
            }],
        )
        .await?;

        Ok(requisition.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<MaterialRequisition> {
        MaterialRequisitionRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("MaterialRequisition"))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: RequisitionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<MaterialRequisition>> {
        MaterialRequisitionRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn confirm(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let requisition = MaterialRequisitionRepo::get_by_id(&mut *db, id)
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
            &mut *db,
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
    /// 设计：issue -> InventoryTransaction.record(MaterialIssue)
    async fn issue(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: IssueMaterialReq,
    ) -> Result<()> {
        let requisition = MaterialRequisitionRepo::get_by_id(&mut *db, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("MaterialRequisition"))?;

        if requisition.status != RequisitionStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", requisition.status),
                to: "Issued".to_string(),
            });
        }

        let existing_items = MaterialRequisitionRepo::get_items(&mut *db, req.id)
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
                &mut *db,
                item.item_id,
                item.issued_qty,
                variance_qty,
                item.bin_id,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            // issue -> InventoryTransaction.record(MaterialIssue)
            new_inventory_transaction_service(self.pool.clone())
            .record(
                ctx, db,
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
            .await?;
        }

        let affected = MaterialRequisitionRepo::update_status(
            &mut *db,
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
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let requisition = MaterialRequisitionRepo::get_by_id(&mut *db, id)
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
            MaterialRequisitionRepo::soft_delete(&mut *db, id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found("MaterialRequisition"));
        }

        Ok(())
    }
}
