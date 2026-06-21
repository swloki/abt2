use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{
    CancelOutsourcingReq, ConvertToInternalReq, CreateOutsourcingOrderReq, OutsourcingMaterial,
    OutsourcingOrder, OutsourcingOrderQuery, ReceiveOutsourcingReq, SendOutsourcingReq,
    UpdateOutsourcingOrderReq, UpdateOutsourcingParams, WorkOrderOutsourcingSummary,
};
use super::repo::{OutsourcingMaterialRepo, OutsourcingOrderRepo};
use super::service::OutsourcingOrderService;
use crate::mes::production_batch::{new_production_batch_service, ProductionBatchService};
use crate::mes::work_order::model::CreateWorkOrderReq;
use crate::mes::work_order::{new_work_order_service, service::WorkOrderService};
use crate::om::enums::{OutsourcingStatus, OutsourcingType};
use crate::om::outsourcing_tracking::model::RecordNodeReq;
use crate::om::outsourcing_tracking::{
    new_outsourcing_tracking_service, service::OutsourcingTrackingService,
};
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus, InspectionType};
use crate::qms::inspection_result::model::{CreateInspectionResultReq, InspectionResultFilter};
use crate::qms::inspection_result::{
    new_inspection_result_service, service::InspectionResultService,
};
use crate::qms::inspection_specification::{
    new_inspection_specification_service, service::InspectionSpecificationService,
};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::{new_cost_entry_service, service::CostEntryService};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_sequence::{
    new_document_sequence_service, service::DocumentSequenceService,
};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::cost::{CostEntityType, CostType};
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::idempotency::{new_idempotency_service, service::IdempotencyService};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::wms::transfer::model::{CreateTransferItemReq, CreateTransferReq};
use crate::wms::transfer::{new_transfer_service, service::TransferService};
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::{new_inventory_transaction_service, service::InventoryTransactionService};
use crate::wms::enums::TransactionType;

const ENTITY_TYPE: &str = "OutsourcingOrder";

pub struct OutsourcingOrderServiceImpl {
    pool: PgPool,
}

impl OutsourcingOrderServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

async fn get_order(db: PgExecutor<'_>, id: i64) -> Result<OutsourcingOrder> {
    OutsourcingOrderRepo::get_by_id(db, id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
}

fn check_version(order: &OutsourcingOrder, expected: i32) -> Result<()> {
    if order.version != expected {
        return Err(DomainError::ConcurrentConflict);
    }
    Ok(())
}

#[async_trait]
impl OutsourcingOrderService for OutsourcingOrderServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateOutsourcingOrderReq,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = crate::shared::idempotency::service::key_to_i64(key);
            if !new_idempotency_service(self.pool.clone())
                .check_and_mark(ctx, db, hash, "OutsourcingOrder:create")
                .await?
            {
                return Err(DomainError::duplicate(ENTITY_TYPE));
            }
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::OutsourcingOrder)
            .await?;

        let id = OutsourcingOrderRepo::insert(&mut *db, &req, &doc_number, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !req.materials.is_empty() {
            let mut seen_products = std::collections::HashSet::new();
            for mat in &req.materials {
                if !seen_products.insert(mat.product_id) {
                    return Err(DomainError::validation(format!(
                        "发料明细中产品 ID {} 重复",
                        mat.product_id
                    )));
                }
            }
            OutsourcingMaterialRepo::insert_batch(&mut *db, id, &req.materials)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        // 登记状态机初始 Draft 状态——缺失则后续 send/receive/cancel/convert 的 transition() 全部失败
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
            .await?;

        Ok(id)
    }

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpdateOutsourcingOrderReq,
    ) -> Result<()> {
        let order = get_order(db, req.id).await?;
        if order.status != OutsourcingStatus::Draft {
            return Err(DomainError::validation("仅 DRAFT 状态可修改"));
        }
        check_version(&order, req.expected_version)?;

        let rows = OutsourcingOrderRepo::update(
            &mut *db,
            req.id,
            req.expected_version,
            &UpdateOutsourcingParams {
                supplier_id: req.supplier_id,
                planned_qty: req.planned_qty,
                unit_price: req.unit_price,
                scheduled_date: req.scheduled_date,
                remark: req.remark.as_deref(),
            },
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        if let Some(materials) = req.materials {
            OutsourcingMaterialRepo::replace_batch(&mut *db, req.id, &materials)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: req.id, action: AuditAction::Update, changes: None, context: None },
            )
            .await?;

        Ok(())
    }

    async fn send(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: SendOutsourcingReq,
    ) -> Result<()> {
        let order = get_order(db, req.id).await?;
        check_version(&order, req.expected_version)?;

        let materials = OutsourcingMaterialRepo::list_by_outsourcing_id(&mut *db, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if materials.is_empty() {
            return Err(DomainError::validation(
                "委外单必须包含至少一项发料明细才能发料",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, req.id, "Sent", req.remark.as_deref())
            .await?;

        let rows = OutsourcingOrderRepo::update_status_and_version(
            &mut *db,
            req.id,
            req.expected_version,
            OutsourcingStatus::Sent,
            "",
            &[],
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // WMS: 发料到虚拟库位 — 创建调拨单并发货
        let transfer_date = chrono::Utc::now().date_naive();
        let transfer_items: Vec<CreateTransferItemReq> = materials
            .iter()
            .map(|m| CreateTransferItemReq {
                product_id: m.product_id,
                quantity: m.planned_qty,
                batch_no: None,
            })
            .collect();
        let mut transfer_ids = Vec::new();
        if !transfer_items.is_empty() {
            let tid = new_transfer_service(self.pool.clone())
                .create(
                    ctx,
                    db,
                    CreateTransferReq {
                        from_warehouse_id: order.source_warehouse_id
                            .ok_or_else(|| DomainError::validation("委外单未设置发料源仓库，无法发料"))?,
                        from_zone_id: None,
                        from_bin_id: None,
                        to_warehouse_id: order.virtual_warehouse_id,
                        to_zone_id: None,
                        to_bin_id: None,
                        transfer_date,
                        items: transfer_items,
                    },
                )
                .await?;
            new_transfer_service(self.pool.clone())
                .dispatch(ctx, db, tid)
                .await?;
            // complete 让虚拟仓实际收到库存（dispatch 仅扣源仓，complete 才入目标仓），
            // 否则后续 receive 从虚拟仓调回时会报"库存数量不能为负"
            new_transfer_service(self.pool.clone())
                .complete(ctx, db, tid)
                .await?;
            transfer_ids.push(tid);
        }

        // 更新材料已发数量
        for mat in &materials {
            OutsourcingMaterialRepo::update_sent_qty(
                &mut *db,
                req.id,
                mat.product_id,
                mat.planned_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 追踪节点: SendMaterial
        let tracking_id = new_outsourcing_tracking_service()
            .record_node(
                ctx,
                db,
                RecordNodeReq {
                    outsourcing_id: req.id,
                    node_type: crate::om::enums::TrackingNodeType::SendMaterial,
                    tracked_at: None,
                    remark: None,
                },
            )
            .await?;

        // 单据关联: OutsourcingOrder → OutsourcingTracking
        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::OutsourcingOrder,
                    source_id: req.id,
                    target_type: DocumentType::OutsourcingTracking,
                    target_id: tracking_id,
                    link_type: LinkType::References,
                }],
            )
            .await?;

        // 审计
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: req.id,
                    action: AuditAction::Transition,
                    changes: Some(json!({ "from": "Draft", "to": "Sent" })),
                    context: None,
                },
            )
            .await?;

        // 领域事件: OutsourcingSent
        let material_ids: Vec<i64> = materials.iter().map(|m| m.id).collect();
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::OutsourcingSent,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: req.id,
                    payload: json!({
                        "outsourcing_id": req.id,
                        "doc_number": order.doc_number,
                        "supplier_id": order.supplier_id,
                        "product_id": order.product_id,
                        "planned_qty": order.planned_qty.to_string(),
                        "virtual_warehouse_id": order.virtual_warehouse_id,
                        "material_ids": material_ids,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 单据关联: OutsourcingOrder → InventoryTransfer
        let mut links: Vec<LinkRequest> = transfer_ids
            .into_iter()
            .map(|tid| LinkRequest {
                source_type: DocumentType::OutsourcingOrder,
                source_id: req.id,
                target_type: DocumentType::InventoryTransfer,
                target_id: tid,
                link_type: LinkType::References,
            })
            .collect();

        // 单据关联: OutsourcingOrder → WorkOrder
        if let Some(wo_id) = order.work_order_id {
            links.push(LinkRequest {
                source_type: DocumentType::OutsourcingOrder,
                source_id: req.id,
                target_type: DocumentType::WorkOrder,
                target_id: wo_id,
                link_type: LinkType::DerivedFrom,
            });
        }
        if !links.is_empty() {
            new_document_link_service(self.pool.clone())
                .create_links(ctx, db, links)
                .await?;
        }

        Ok(())
    }

    async fn receive(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ReceiveOutsourcingReq,
    ) -> Result<()> {
        let order = get_order(db, req.id).await?;
        check_version(&order, req.expected_version)?;

        new_state_machine_service(self.pool.clone())
            .transition(
                ctx,
                db,
                ENTITY_TYPE,
                req.id,
                "Received",
                req.remark.as_deref(),
            )
            .await?;

        // QMS: 查产品 IQC 规范，有则创建检验结果（无规范时跳过，质量门禁按无结果判定通过）
        let iqc_qty = req.iqc_passed_qty.unwrap_or(req.received_qty);
        let iqc_spec = new_inspection_specification_service(self.pool.clone())
            .find_by_product_and_type(ctx, db, order.product_id, InspectionType::Iqc)
            .await?;
        if let Some(spec) = iqc_spec {
            new_inspection_result_service(self.pool.clone())
                .create(
                    ctx,
                    db,
                    CreateInspectionResultReq {
                        spec_id: spec.id,
                        source_type: InspectionSourceType::OutsourcingOrder,
                        source_id: req.id,
                        batch_no: String::new(),
                        sample_qty: req.received_qty,
                    },
                )
                .await?;
        }

        // QMS: 质量门禁检查 — 查询检验结果
        let qms_results = new_inspection_result_service(self.pool.clone())
            .list_by_source(
                ctx,
                db,
                InspectionResultFilter {
                    source_type: Some(InspectionSourceType::OutsourcingOrder),
                    source_id: Some(req.id),
                    ..Default::default()
                },
                PageParams {
                    page: 1,
                    page_size: 100,
                },
            )
            .await?;

        let iqc_passed = qms_results.items.is_empty()
            || qms_results.items.iter().all(|r| {
                r.status == InspectionStatus::Completed && r.result == InspectionResultType::Pass
            });
        if !iqc_passed {
            return Err(DomainError::business_rule("IQC 检验未通过，无法入库"));
        }

        let rows = OutsourcingOrderRepo::update_completed_qty(
            &mut *db,
            req.id,
            req.expected_version,
            OutsourcingStatus::Received,
            iqc_qty,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // WMS 收货库存：委外产品入库到收货仓 + 发料物料从虚拟仓消耗（供应商已加工用掉）
        let warehouse_id = req
            .warehouse_id
            .ok_or_else(|| DomainError::validation("收货仓库 ID 不能为空"))?;
        let materials = OutsourcingMaterialRepo::list_by_outsourcing_id(db, req.id).await?;
        let tx_svc = new_inventory_transaction_service(self.pool.clone());

        // 1. 委外产品入库（+received_qty 到收货仓）
        tx_svc
            .record(
                ctx,
                db,
                RecordTransactionReq {
                    doc_number: Some(order.doc_number.clone()),
                    delivery_no: None,
                    source_doc_number: None,
                    transaction_type: TransactionType::ProductionReceipt,
                    product_id: order.product_id,
                    warehouse_id,
                    zone_id: None,
                    bin_id: None,
                    batch_no: None,
                    quantity: iqc_qty,
                    unit_cost: Some(order.unit_price),
                    source_type: "outsourcing_order".to_string(),
                    source_id: req.id,
                    remark: Some("委外收货-产品入库".to_string()),
                },
            )
            .await?;

        // 2. 发料物料从虚拟仓消耗（-sent_qty，供应商已用掉）
        for mat in &materials {
            if mat.sent_qty > Decimal::ZERO {
                tx_svc
                    .record(
                        ctx,
                        db,
                        RecordTransactionReq {
                            doc_number: Some(order.doc_number.clone()),
                            delivery_no: None,
                            source_doc_number: None,
                            transaction_type: TransactionType::MaterialIssue,
                            product_id: mat.product_id,
                            warehouse_id: order.virtual_warehouse_id,
                            zone_id: None,
                            bin_id: None,
                            batch_no: None,
                            quantity: -mat.sent_qty,
                            unit_cost: Some(mat.unit_cost),
                            source_type: "outsourcing_order".to_string(),
                            source_id: req.id,
                            remark: Some("委外收货-发料物料消耗".to_string()),
                        },
                    )
                    .await?;
            }
        }

        // 追踪节点: IqcInspected → Warehoused
        new_outsourcing_tracking_service()
            .record_node(
                ctx,
                db,
                RecordNodeReq {
                    outsourcing_id: req.id,
                    node_type: crate::om::enums::TrackingNodeType::IqcInspected,
                    tracked_at: None,
                    remark: Some(format!("IQC 检验通过，合格数量: {}", iqc_qty)),
                },
            )
            .await?;
        let tracking_id = new_outsourcing_tracking_service()
            .record_node(
                ctx,
                db,
                RecordNodeReq {
                    outsourcing_id: req.id,
                    node_type: crate::om::enums::TrackingNodeType::Warehoused,
                    tracked_at: None,
                    remark: None,
                },
            )
            .await?;

        // 单据关联: OutsourcingOrder → OutsourcingTracking + InventoryTransfer
        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::OutsourcingOrder,
                    source_id: req.id,
                    target_type: DocumentType::OutsourcingTracking,
                    target_id: tracking_id,
                    link_type: LinkType::References,
                }],
            )
            .await?;

        // 成本分录: 外协收货时记外协成本（借:在制品 / 贷:应付外协费）
        let outsourcing_cost = iqc_qty * order.unit_price;
        let period = chrono::Utc::now().format("%Y-%m").to_string();
        new_cost_entry_service(self.pool.clone())
            .create_entries(
                ctx,
                db,
                vec![
                    EntryRequest {
                        entity_type: CostEntityType::OutsourcingOrder,
                        entity_id: req.id,
                        cost_type: CostType::Outsource,
                        debit_amount: outsourcing_cost,
                        credit_amount: Decimal::ZERO,
                        cost_center: None,
                        profit_center: None,
                        period: period.clone(),
                        source_type: DocumentType::OutsourcingOrder,
                        source_id: req.id,
                    },
                    EntryRequest {
                        entity_type: CostEntityType::OutsourcingOrder,
                        entity_id: req.id,
                        cost_type: CostType::Outsource,
                        debit_amount: Decimal::ZERO,
                        credit_amount: outsourcing_cost,
                        cost_center: None,
                        profit_center: None,
                        period,
                        source_type: DocumentType::OutsourcingOrder,
                        source_id: req.id,
                    },
                ],
            )
            .await?;

        // 审计
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: req.id,
                    action: AuditAction::Transition,
                    changes: Some(json!({ "from": format!("{:?}", order.status), "to": "Received", "received_qty": req.received_qty.to_string(), "iqc_passed_qty": iqc_qty.to_string() })),
                    context: None,
                },
            )
            .await?;

        // 领域事件: OutsourcingReceived
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::OutsourcingReceived,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: req.id,
                    payload: json!({
                        "outsourcing_id": req.id,
                        "doc_number": order.doc_number,
                        "received_qty": req.received_qty.to_string(),
                        "iqc_passed_qty": iqc_qty.to_string(),
                        "warehouse_id": warehouse_id,
                        "supplier_id": order.supplier_id,
                        "unit_price": order.unit_price.to_string(),
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn convert_to_internal(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ConvertToInternalReq,
    ) -> Result<i64> {
        let order = get_order(db, req.id).await?;
        check_version(&order, req.expected_version)?;

        if !matches!(
            order.status,
            OutsourcingStatus::Draft | OutsourcingStatus::Sent
        ) {
            return Err(DomainError::validation("仅 DRAFT 或 SENT 状态可转为自制"));
        }
        if !matches!(
            order.outsourcing_type,
            OutsourcingType::Full | OutsourcingType::Process
        ) {
            return Err(DomainError::business_rule("仅 FULL/PROCESS 类型可转为自制"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(
                ctx,
                db,
                ENTITY_TYPE,
                req.id,
                "ConvertedToInternal",
                req.remark.as_deref(),
            )
            .await?;

        let rows = OutsourcingOrderRepo::update_status_and_version(
            &mut *db,
            req.id,
            req.expected_version,
            OutsourcingStatus::ConvertedToInternal,
            "",
            &[],
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // MES: 创建内部工单
        let scheduled_start = chrono::Local::now().date_naive();
        let scheduled_end = scheduled_start + chrono::Duration::days(7);
        let new_wo_id = new_work_order_service(self.pool.clone())
            .create(
                ctx,
                db,
                CreateWorkOrderReq {
                    plan_item_id: None,
                    product_id: order.product_id,
                    bom_snapshot_id: None,
                    routing_id: None,
                    planned_qty: order.planned_qty,
                    scheduled_start,
                    scheduled_end,
                    work_center_id: None,
                    sales_order_id: None,
                    remark: Some(format!("从委外单 #{} 转自制", req.id)),
                },
            )
            .await?;

        // 获取原始工单的仓库信息
        let wo = if let Some(orig_wo_id) = order.work_order_id {
            new_work_order_service(self.pool.clone())
                .find_by_id(ctx, db, orig_wo_id)
                .await
                .ok()
        } else {
            None
        };
        let return_warehouse_id = wo.and_then(|w| w.work_center_id).unwrap_or(0);

        // WMS: 材料回调 — 创建调拨单、发货、完成
        let materials = OutsourcingMaterialRepo::list_by_outsourcing_id(&mut *db, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let in_transit_items: Vec<CreateTransferItemReq> = materials
            .iter()
            .filter(|m| m.in_transit_qty() > Decimal::ZERO)
            .map(|m| CreateTransferItemReq {
                product_id: m.product_id,
                quantity: m.in_transit_qty(),
                batch_no: None,
            })
            .collect();
        let mut convert_transfer_id = None;
        if !in_transit_items.is_empty() {
            let transfer_date = chrono::Utc::now().date_naive();
            let tid = new_transfer_service(self.pool.clone())
                .create(
                    ctx,
                    db,
                    CreateTransferReq {
                        from_warehouse_id: order.virtual_warehouse_id,
                        from_zone_id: None,
                        from_bin_id: None,
                        to_warehouse_id: return_warehouse_id,
                        to_zone_id: None,
                        to_bin_id: None,
                        transfer_date,
                        items: in_transit_items,
                    },
                )
                .await?;
            new_transfer_service(self.pool.clone())
                .dispatch(ctx, db, tid)
                .await?;
            new_transfer_service(self.pool.clone())
                .complete(ctx, db, tid)
                .await?;
            convert_transfer_id = Some(tid);
        }

        // 单据关联: OutsourcingOrder → InventoryTransfer + WorkOrder
        let mut convert_links: Vec<LinkRequest> = Vec::new();
        if let Some(tid) = convert_transfer_id {
            convert_links.push(LinkRequest {
                source_type: DocumentType::OutsourcingOrder,
                source_id: req.id,
                target_type: DocumentType::InventoryTransfer,
                target_id: tid,
                link_type: LinkType::References,
            });
        }
        convert_links.push(LinkRequest {
            source_type: DocumentType::OutsourcingOrder,
            source_id: req.id,
            target_type: DocumentType::WorkOrder,
            target_id: new_wo_id,
            link_type: LinkType::DerivedFrom,
        });
        if !convert_links.is_empty() {
            new_document_link_service(self.pool.clone())
                .create_links(ctx, db, convert_links)
                .await?;
        }

        // 审计
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: req.id,
                    action: AuditAction::Transition,
                    changes: Some(json!({ "from": format!("{:?}", order.status), "to": "ConvertedToInternal", "new_work_order_id": new_wo_id })),
                    context: None,
                },
            )
            .await?;

        // 领域事件: OutsourcingConvertedToInternal
        let remaining_materials: Vec<serde_json::Value> = materials
            .iter()
            .filter(|m| m.in_transit_qty() > Decimal::ZERO)
            .map(|m| {
                json!({
                    "material_id": m.id,
                    "product_id": m.product_id,
                    "remaining_qty": m.in_transit_qty().to_string(),
                })
            })
            .collect();
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::ConvertedToInternal,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: req.id,
                    payload: json!({
                        "outsourcing_id": req.id,
                        "doc_number": order.doc_number,
                        "new_work_order_id": new_wo_id,
                        "product_id": order.product_id,
                        "planned_qty": order.planned_qty.to_string(),
                        "remaining_materials": remaining_materials,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(new_wo_id)
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CancelOutsourcingReq,
    ) -> Result<()> {
        let order = get_order(db, req.id).await?;
        if order.status != OutsourcingStatus::Draft {
            return Err(DomainError::validation("仅 DRAFT 状态可取消"));
        }
        check_version(&order, req.expected_version)?;

        new_state_machine_service(self.pool.clone())
            .transition(
                ctx,
                db,
                ENTITY_TYPE,
                req.id,
                "Cancelled",
                req.remark.as_deref(),
            )
            .await?;

        let rows = OutsourcingOrderRepo::update_status_and_version(
            &mut *db,
            req.id,
            req.expected_version,
            OutsourcingStatus::Cancelled,
            "",
            &[],
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 审计
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: req.id,
                    action: AuditAction::Transition,
                    changes: Some(json!({ "from": "Draft", "to": "Cancelled" })),
                    context: None,
                },
            )
            .await?;

        // 领域事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::OutsourcingCancelled,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: req.id,
                    payload: json!({ "doc_number": order.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<OutsourcingOrder> {
        get_order(db, id).await
    }

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: OutsourcingOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<OutsourcingOrder>> {
        let scope = (ctx.data_scope, ctx.operator_id, ctx.department_id);
        let (items, total) = OutsourcingOrderRepo::query(&mut *db, &filter, &page, scope)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(
            items,
            total,
            page.page,
            page.page_size,
        ))
    }

    async fn list_materials(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outsourcing_id: i64,
    ) -> Result<Vec<OutsourcingMaterial>> {
        OutsourcingMaterialRepo::list_by_outsourcing_id(db, outsourcing_id).await
    }

    async fn list_inventory_records(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outsourcing_id: i64,
    ) -> Result<Vec<crate::wms::inventory_transaction::model::InventoryTransaction>> {
        use std::collections::HashSet;

        // 委外单 → 关联的调拨单 ids（send/receive 已建立 OutsourcingOrder→InventoryTransfer link）
        let links = new_document_link_service(self.pool.clone())
            .find_linked(
                ctx,
                db,
                DocumentType::OutsourcingOrder,
                outsourcing_id,
                1,
                200,
            )
            .await?;
        let transfer_ids: HashSet<i64> = links
            .items
            .into_iter()
            .filter(|l| l.target_type == DocumentType::InventoryTransfer)
            .map(|l| l.target_id)
            .collect();

        // 每个调拨单 → 库存流水
        let tx_svc = new_inventory_transaction_service(self.pool.clone());
        // 委外单直接关联的流水（receive 产品入库/物料消耗，source_type=outsourcing_order）
        let mut records = tx_svc
            .find_by_source(ctx, db, "outsourcing_order", outsourcing_id)
            .await?;
        for tid in transfer_ids {
            let txns = tx_svc
                .find_by_source(ctx, db, "inventory_transfer", tid)
                .await?;
            records.extend(txns);
        }

        // 按时间升序 + 去重
        records.sort_by_key(|r| r.created_at);
        let mut seen = HashSet::new();
        Ok(records.into_iter().filter(|r| seen.insert(r.id)).collect())
    }

    async fn outsourcing_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderOutsourcingSummary> {
        use crate::master_data::product::{new_product_service, ProductService};
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, db, work_order_id)
            .await?;
        let product_name = new_product_service(self.pool.clone())
            .get(ctx, db, wo.product_id)
            .await
            .map(|p| p.pdt_name)
            .unwrap_or_default();
        let routings = new_production_batch_service(self.pool.clone())
            .list_routings(ctx, db, work_order_id)
            .await?;
        Ok(WorkOrderOutsourcingSummary {
            product_id: wo.product_id,
            product_name,
            planned_qty: wo.planned_qty,
            scheduled_end: wo.scheduled_end,
            customer_name: wo.source_customer,
            routings,
        })
    }
}
