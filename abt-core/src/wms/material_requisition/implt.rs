use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{CreateManualReq, IssueMaterialReq, MaterialRequisition, RequisitionFilter};
use super::repo::MaterialRequisitionRepo;
use super::service::MaterialRequisitionService;
use crate::mes::work_order::{new_work_order_service, service::WorkOrderService};
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::wms::backflush::resolve_warehouse_id;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::types::PgExecutor;
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::{AuditAction, CostEntityType, CostType, DocumentType, LinkType};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::RequisitionStatus;
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::{new_inventory_transaction_service, service::InventoryTransactionService};
use crate::shared::cost_entry::{new_cost_entry_service, model::EntryRequest, service::CostEntryService};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};

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

        // 确定仓库（V1：回退到第一个活跃仓库）
        let warehouse_id = resolve_warehouse_id(db).await?;

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

        // 从 BOM 快照展开叶子组件 → 生成领料单明细行
        if let Some(snapshot_id) = wo.bom_snapshot_id {
            let snapshot_opt = new_bom_query_service(self.pool.clone())
                .get_snapshot_by_id(ctx, db, snapshot_id).await?;

            if let Some(snapshot) = snapshot_opt {
                let all_nodes = &snapshot.bom_detail.nodes;
                let parent_ids: std::collections::HashSet<i64> =
                    all_nodes.iter().map(|n| n.parent_id).collect();
                let leaf_nodes: Vec<&crate::master_data::bom::model::BomNode> = all_nodes
                    .iter()
                    .filter(|n| !parent_ids.contains(&n.id))
                    .collect();

                for node in &leaf_nodes {
                    let required_qty = node.quantity * wo.planned_qty;
                    MaterialRequisitionRepo::insert_item(
                        &mut *db,
                        requisition.id,
                        node.product_id,
                        required_qty,
                    )
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }

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

        // 领料出库 → 创建材料成本分录（工单关联时）
        if requisition.work_order_id > 0 {
            let period = chrono::Local::now().format("%Y-%m").to_string();
            let total_issued: Decimal = req.items.iter().map(|i| i.issued_qty).sum();
            new_cost_entry_service(self.pool.clone())
                .create_entries(
                    ctx, db,
                    vec![EntryRequest {
                        entity_type: CostEntityType::WorkOrder,
                        entity_id: requisition.work_order_id,
                        cost_type: CostType::Material,
                        debit_amount: total_issued,
                        credit_amount: total_issued,
                        cost_center: None,
                        profit_center: None,
                        period,
                        source_type: DocumentType::MaterialRequisition,
                        source_id: req.id,
                    }],
                )
                .await?;
        }

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("MaterialRequisition", req.id, AuditAction::Transition),
            )
            .await?;

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

        let affected = MaterialRequisitionRepo::update_status(
            &mut *db,
            id,
            RequisitionStatus::Cancelled,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found("MaterialRequisition"));
        }

        Ok(())
    }

    /// 手动创建领料单（非工单驱动，work_order_id = 0）
    async fn create_manual(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateManualReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::validation("请至少添加一条领料明细"));
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::MaterialRequisition)
            .await
            .unwrap_or_else(|_| format!("MR{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let requisition = MaterialRequisitionRepo::insert(
            &mut *db,
            &doc_number,
            0, // work_order_id = 0 表示手动创建
            req.requisition_date,
            req.warehouse_id,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        for item in &req.items {
            MaterialRequisitionRepo::insert_item(
                &mut *db,
                requisition.id,
                item.product_id,
                item.requested_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(requisition.id)
    }
}
