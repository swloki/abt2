use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{CreateManualReq, IssueMaterialReq, MaterialRequisition, RequisitionFilter, ReturnMaterialReq};
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
use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
use crate::wms::stock_ledger::repo::StockLedgerRepo;

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

        // 校验：BOM 快照必须存在（对标 Odoo: MO 必须有 BOM 才能产生 move）
        let snapshot_id = wo.bom_snapshot_id.ok_or_else(|| {
            DomainError::BusinessRule(
                "工单无 BOM 快照，请先确保 release 时 BOM 快照创建成功".into(),
            )
        })?;
        let snapshot = new_bom_query_service(self.pool.clone())
            .get_snapshot_by_id(ctx, db, snapshot_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomSnapshot"))?;

        // 从 BOM 快照展开叶子组件 → 生成领料单明细行
        let leaf_nodes = snapshot.bom_detail.leaf_nodes();
        for node in &leaf_nodes {
            let required_qty = node.quantity * wo.planned_qty;
            MaterialRequisitionRepo::insert_item(
                &mut *db,
                requisition.id,
                node.product_id,
                required_qty,
                None,
                None,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
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

        if requisition.status != RequisitionStatus::Confirmed
            && requisition.status != RequisitionStatus::PartiallyIssued
        {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", requisition.status),
                to: "Issued".to_string(),
            });
        }

        let existing_items = MaterialRequisitionRepo::get_items(&mut *db, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 批量预加载本单涉及产品的最后已知单位成本（消除循环内 N+1）
        let cost_product_ids: Vec<i64> = req
            .items
            .iter()
            .filter_map(|item| {
                existing_items
                    .iter()
                    .find(|i| i.id == item.item_id)
                    .map(|i| i.product_id)
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let unit_cost_map =
            StockLedgerRepo::last_known_unit_cost_batch(&mut *db, &cost_product_ids)
                .await
                .unwrap_or_default();

        let mut total_cost_amount = Decimal::ZERO;

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

            // 单位成本取自预加载 map（stock_ledger 最后已知成本，无则 0）
            let unit_cost = unit_cost_map
                .get(&found_item.product_id)
                .copied()
                .unwrap_or(Decimal::ZERO);

            total_cost_amount += item.issued_qty * unit_cost;

            // issue -> InventoryTransaction.record(MaterialIssue)
            new_inventory_transaction_service(self.pool.clone())
                .record(
                    ctx, db,
                    RecordTransactionReq {
                        doc_number: None,
                        delivery_no: None,
                        source_doc_number: None,
                        transaction_type: crate::wms::enums::TransactionType::MaterialIssue,
                        product_id: found_item.product_id,
                        warehouse_id: requisition.warehouse_id,
                        zone_id: None,
                        bin_id: item.bin_id,
                        batch_no: None,
                        quantity: -item.issued_qty,
                        unit_cost: Some(unit_cost),
                        source_type: "material_requisition".to_string(),
                        source_id: req.id,
                        remark: None,
                    },
                )
                .await?;

            // 消耗库存预留（对标 Odoo move._action_done 消费 reservation）
            if requisition.work_order_id > 0 {
                new_inventory_reservation_service(self.pool.clone())
                    .consume(
                        ctx,
                        db,
                        DocumentType::WorkOrder,
                        requisition.work_order_id,
                        found_item.product_id,
                        item.issued_qty,
                    )
                    .await?;
            }
        }

        // 判断是否全部发完 → PartiallyIssued or Issued
        let issued_map: std::collections::HashMap<i64, Decimal> =
            req.items.iter().map(|r| (r.item_id, r.issued_qty)).collect();
        let all_fully_issued = existing_items.iter().all(|i| {
            let issued = issued_map.get(&i.id).copied().unwrap_or(i.issued_qty);
            issued >= i.requested_qty
        });
        let new_status = if all_fully_issued {
            RequisitionStatus::Issued
        } else {
            RequisitionStatus::PartiallyIssued
        };

        let affected = MaterialRequisitionRepo::update_status(
            &mut *db,
            req.id,
            new_status,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found("MaterialRequisition"));
        }

        // 领料出库 → 创建材料成本分录（真实金额 = qty × unit_cost）
        if requisition.work_order_id > 0 && total_cost_amount > Decimal::ZERO {
            let period = chrono::Local::now().format("%Y-%m").to_string();
            new_cost_entry_service(self.pool.clone())
                .create_entries(
                    ctx, db,
                    vec![EntryRequest {
                        entity_type: CostEntityType::WorkOrder,
                        entity_id: requisition.work_order_id,
                        cost_type: CostType::Material,
                        debit_amount: total_cost_amount,
                        credit_amount: total_cost_amount,
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
            && requisition.status != RequisitionStatus::PartiallyIssued
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

    /// 退料：Issued/PartiallyIssued → 退料入库（对标 Odoo stock.move.reverse）
    async fn return_materials(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ReturnMaterialReq,
    ) -> Result<()> {
        let requisition = MaterialRequisitionRepo::get_by_id(&mut *db, req.requisition_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("MaterialRequisition"))?;

        if requisition.status != RequisitionStatus::Issued
            && requisition.status != RequisitionStatus::PartiallyIssued
        {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", requisition.status),
                to: "Returned".to_string(),
            });
        }

        let existing_items = MaterialRequisitionRepo::get_items(&mut *db, req.requisition_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        for item in &req.items {
            let found = existing_items.iter().find(|i| i.id == item.item_id);
            let Some(found_item) = found else {
                return Err(DomainError::not_found(format!(
                    "MaterialReqItem {}",
                    item.item_id
                )));
            };

            if item.return_qty > found_item.issued_qty {
                return Err(DomainError::validation(format!(
                    "退料量 {} 超过已发料量 {}",
                    item.return_qty, found_item.issued_qty
                )));
            }

            // 库存交易：退料入库（正数 = 入库）
            new_inventory_transaction_service(self.pool.clone())
                .record(
                    ctx, db,
                    RecordTransactionReq {
                        doc_number: None,
                        delivery_no: None,
                        source_doc_number: None,
                        transaction_type: crate::wms::enums::TransactionType::MaterialIssue,
                        product_id: found_item.product_id,
                        warehouse_id: requisition.warehouse_id,
                        zone_id: None,
                        bin_id: item.bin_id,
                        batch_no: None,
                        quantity: item.return_qty,
                        unit_cost: None,
                        source_type: "material_return".to_string(),
                        source_id: req.requisition_id,
                        remark: Some(req.reason.clone()),
                    },
                )
                .await?;

            // 更新 issued_qty（扣减退料量）
            let new_issued_qty = found_item.issued_qty - item.return_qty;
            let new_variance = new_issued_qty - found_item.requested_qty;
            MaterialRequisitionRepo::update_item_issued(
                &mut *db,
                item.item_id,
                new_issued_qty,
                new_variance,
                item.bin_id,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new(
                    "MaterialRequisition",
                    req.requisition_id,
                    AuditAction::Update,
                ),
            )
            .await?;

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
                None,
                None,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(requisition.id)
    }
}
