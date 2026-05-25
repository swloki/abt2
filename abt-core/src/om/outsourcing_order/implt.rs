use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{
    CancelOutsourcingReq, ConvertToInternalReq, CreateOutsourcingOrderReq, OutsourcingOrder,
    OutsourcingOrderQuery, ReceiveOutsourcingReq, SendOutsourcingReq, UpdateOutsourcingOrderReq,
};
use super::repo::{OutsourcingMaterialRepo, OutsourcingOrderRepo};
use super::service::OutsourcingOrderService;
use crate::om::enums::{OutsourcingStatus, OutsourcingType};
use crate::om::outsourcing_tracking::model::RecordNodeReq;
use crate::om::outsourcing_tracking::service::OutsourcingTrackingService;
use crate::mes::work_order::model::CreateWorkOrderReq;
use crate::mes::work_order::service::WorkOrderService;
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
use crate::qms::inspection_result::model::{CreateInspectionResultReq, InspectionResultFilter};
use crate::qms::inspection_result::service::InspectionResultService;
use crate::wms::transfer::model::{CreateTransferItemReq, CreateTransferReq};
use crate::wms::transfer::service::TransferService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::service::CostEntryService;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::service::DocumentLinkService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::cost::{CostEntityType, CostType};
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::idempotency::service::{key_to_i64, IdempotencyService};
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "OutsourcingOrder";

pub struct OutsourcingOrderServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    doc_link: Arc<dyn DocumentLinkService>,
    cost_entry: Arc<dyn CostEntryService>,
    idempotency: Arc<dyn IdempotencyService>,
    tracking_service: Arc<dyn OutsourcingTrackingService>,
    transfer_service: Arc<dyn TransferService>,
    qms: Arc<dyn InspectionResultService>,
    work_order: Arc<dyn WorkOrderService>,
}

impl OutsourcingOrderServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
        doc_link: Arc<dyn DocumentLinkService>,
        cost_entry: Arc<dyn CostEntryService>,
        idempotency: Arc<dyn IdempotencyService>,
        tracking_service: Arc<dyn OutsourcingTrackingService>,
        transfer_service: Arc<dyn TransferService>,
        qms: Arc<dyn InspectionResultService>,
        work_order: Arc<dyn WorkOrderService>,
    ) -> Self {
        Self {
            pool,
            doc_seq,
            state_machine,
            event_bus,
            audit_log,
            doc_link,
            cost_entry,
            idempotency,
            tracking_service,
            transfer_service,
            qms,
            work_order,
        }
    }
}

async fn get_order(
    ctx: &mut ServiceContext<'_>,
    id: i64,
) -> Result<OutsourcingOrder, DomainError> {
    OutsourcingOrderRepo::get_by_id(&mut *ctx.executor, id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
}

fn check_version(order: &OutsourcingOrder, expected: i32) -> Result<(), DomainError> {
    if order.version != expected {
        return Err(DomainError::ConcurrentConflict);
    }
    Ok(())
}

#[async_trait]
impl OutsourcingOrderService for OutsourcingOrderServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateOutsourcingOrderReq,
        idempotency_key: Option<String>,
    ) -> Result<i64, DomainError> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self
                .idempotency
                .check_and_mark(ctx.reborrow(), hash, "OutsourcingOrder:create")
                .await?
            {
                return Err(DomainError::duplicate(ENTITY_TYPE));
            }
        }

        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::OutsourcingOrder)
            .await?;

        let id = OutsourcingOrderRepo::insert(&mut *ctx.executor, &req, &doc_number, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !req.materials.is_empty() {
            let mut seen_products = std::collections::HashSet::new();
            for mat in &req.materials {
                if !seen_products.insert(mat.product_id) {
                    return Err(DomainError::validation(format!(
                        "发料明细中产品 ID {} 重复", mat.product_id
                    )));
                }
            }
            OutsourcingMaterialRepo::insert_batch(&mut *ctx.executor, id, &req.materials)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        self.audit_log
            .record(
                ctx.reborrow(),
                ENTITY_TYPE,
                id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(id)
    }

    async fn update(
        &self,
        mut ctx: ServiceContext<'_>,
        req: UpdateOutsourcingOrderReq,
    ) -> Result<(), DomainError> {
        let order = get_order(&mut ctx.reborrow(), req.id).await?;
        if order.status != OutsourcingStatus::Draft {
            return Err(DomainError::validation("仅 DRAFT 状态可修改"));
        }
        check_version(&order, req.expected_version)?;

        let rows = OutsourcingOrderRepo::update(
            &mut *ctx.executor,
            req.id,
            req.expected_version,
            req.supplier_id,
            req.planned_qty,
            req.unit_price,
            req.scheduled_date,
            req.remark.as_deref(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        if let Some(materials) = req.materials {
            OutsourcingMaterialRepo::replace_batch(&mut *ctx.executor, req.id, &materials)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        self.audit_log
            .record(
                ctx,
                ENTITY_TYPE,
                req.id,
                AuditAction::Update,
                None,
                None,
            )
            .await?;

        Ok(())
    }

    async fn send(
        &self,
        mut ctx: ServiceContext<'_>,
        req: SendOutsourcingReq,
    ) -> Result<(), DomainError> {
        let order = get_order(&mut ctx.reborrow(), req.id).await?;
        check_version(&order, req.expected_version)?;

        let materials = OutsourcingMaterialRepo::list_by_outsourcing_id(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if materials.is_empty() {
            return Err(DomainError::validation("委外单必须包含至少一项发料明细才能发料"));
        }

        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, req.id, "Sent", req.remark.as_deref())
            .await?;

        let rows = OutsourcingOrderRepo::update_status_and_version(
            &mut *ctx.executor,
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
            let tid = self.transfer_service
                .create(
                    ctx.reborrow(),
                    CreateTransferReq {
                        from_warehouse_id: 0,
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
            self.transfer_service.dispatch(ctx.reborrow(), tid).await?;
            transfer_ids.push(tid);
        }

        // 更新材料已发数量
        for mat in &materials {
            OutsourcingMaterialRepo::update_sent_qty(
                &mut *ctx.executor,
                req.id,
                mat.product_id,
                mat.planned_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 追踪节点: SendMaterial
        let tracking_id = self.tracking_service
            .record_node(
                ctx.reborrow(),
                RecordNodeReq {
                    outsourcing_id: req.id,
                    node_type: crate::om::enums::TrackingNodeType::SendMaterial,
                    tracked_at: None,
                    remark: None,
                },
            )
            .await?;

        // 单据关联: OutsourcingOrder → OutsourcingTracking
        self.doc_link
            .create_links(
                ctx.reborrow(),
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
        self.audit_log
            .record(
                ctx.reborrow(),
                ENTITY_TYPE,
                req.id,
                AuditAction::Transition,
                Some(json!({ "from": "Draft", "to": "Sent" })),
                None,
            )
            .await?;

        // 领域事件: OutsourcingSent
        let material_ids: Vec<i64> = materials.iter().map(|m| m.id).collect();
        self.event_bus
            .publish(
                ctx.reborrow(),
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
            self.doc_link.create_links(ctx, links).await?;
        }

        Ok(())
    }

    async fn receive(
        &self,
        mut ctx: ServiceContext<'_>,
        req: ReceiveOutsourcingReq,
    ) -> Result<(), DomainError> {
        let order = get_order(&mut ctx.reborrow(), req.id).await?;
        check_version(&order, req.expected_version)?;

        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, req.id, "Received", req.remark.as_deref())
            .await?;

        // QMS: 创建 IQC 检验结果
        let iqc_qty = req.iqc_passed_qty.unwrap_or(req.received_qty);
        self.qms.create(
            ctx.reborrow(),
            CreateInspectionResultReq {
                spec_id: 0,
                source_type: InspectionSourceType::OutsourcingOrder,
                source_id: req.id,
                batch_no: String::new(),
                sample_qty: req.received_qty,
            },
        )
        .await?;

        // QMS: 质量门禁检查 — 查询检验结果
        let qms_results = self.qms.list_by_source(
            ctx.reborrow(),
            InspectionResultFilter {
                source_type: Some(InspectionSourceType::OutsourcingOrder),
                source_id: Some(req.id),
                ..Default::default()
            },
            PageParams { page: 1, page_size: 100 },
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
            &mut *ctx.executor,
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

        // WMS: 从虚拟库位调回 — 创建调拨单、发货、完成
        let transfer_date = chrono::Utc::now().date_naive();
        let warehouse_id = req.warehouse_id
            .ok_or_else(|| DomainError::validation("收货仓库 ID 不能为空"))?;
        let transfer_id = self.transfer_service
            .create(
                ctx.reborrow(),
                CreateTransferReq {
                    from_warehouse_id: order.virtual_warehouse_id,
                    from_zone_id: None,
                    from_bin_id: None,
                    to_warehouse_id: warehouse_id,
                    to_zone_id: None,
                    to_bin_id: None,
                    transfer_date,
                    items: vec![CreateTransferItemReq {
                        product_id: order.product_id,
                        quantity: iqc_qty,
                        batch_no: None,
                    }],
                },
            )
            .await?;
        self.transfer_service.dispatch(ctx.reborrow(), transfer_id).await?;
        self.transfer_service.complete(ctx.reborrow(), transfer_id).await?;

        // 追踪节点: IqcInspected → Warehoused
        self.tracking_service
            .record_node(
                ctx.reborrow(),
                RecordNodeReq {
                    outsourcing_id: req.id,
                    node_type: crate::om::enums::TrackingNodeType::IqcInspected,
                    tracked_at: None,
                    remark: Some(format!("IQC 检验通过，合格数量: {}", iqc_qty)),
                },
            )
            .await?;
        let tracking_id = self.tracking_service
            .record_node(
                ctx.reborrow(),
                RecordNodeReq {
                    outsourcing_id: req.id,
                    node_type: crate::om::enums::TrackingNodeType::Warehoused,
                    tracked_at: None,
                    remark: None,
                },
            )
            .await?;

        // 单据关联: OutsourcingOrder → OutsourcingTracking + InventoryTransfer
        self.doc_link
            .create_links(
                ctx.reborrow(),
                vec![
                    LinkRequest {
                        source_type: DocumentType::OutsourcingOrder,
                        source_id: req.id,
                        target_type: DocumentType::OutsourcingTracking,
                        target_id: tracking_id,
                        link_type: LinkType::References,
                    },
                    LinkRequest {
                        source_type: DocumentType::OutsourcingOrder,
                        source_id: req.id,
                        target_type: DocumentType::InventoryTransfer,
                        target_id: transfer_id,
                        link_type: LinkType::References,
                    },
                ],
            )
            .await?;

        // 成本分录: 外协收货时记外协成本（借:在制品 / 贷:应付外协费）
        let outsourcing_cost = iqc_qty * order.unit_price;
        let period = chrono::Utc::now().format("%Y-%m").to_string();
        self.cost_entry
            .create_entries(
                ctx.reborrow(),
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
        self.audit_log
            .record(
                ctx.reborrow(),
                ENTITY_TYPE,
                req.id,
                AuditAction::Transition,
                Some(json!({ "from": format!("{:?}", order.status), "to": "Received", "received_qty": req.received_qty.to_string(), "iqc_passed_qty": iqc_qty.to_string() })),
                None,
            )
            .await?;

        // 领域事件: OutsourcingReceived
        self.event_bus
            .publish(
                ctx,
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
        mut ctx: ServiceContext<'_>,
        req: ConvertToInternalReq,
    ) -> Result<i64, DomainError> {
        let order = get_order(&mut ctx.reborrow(), req.id).await?;
        check_version(&order, req.expected_version)?;

        if !matches!(order.status, OutsourcingStatus::Draft | OutsourcingStatus::Sent) {
            return Err(DomainError::validation("仅 DRAFT 或 SENT 状态可转为自制"));
        }
        if !matches!(order.outsourcing_type, OutsourcingType::Full | OutsourcingType::Process) {
            return Err(DomainError::business_rule(
                "仅 FULL/PROCESS 类型可转为自制",
            ));
        }

        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, req.id, "ConvertedToInternal", req.remark.as_deref())
            .await?;

        let rows = OutsourcingOrderRepo::update_status_and_version(
            &mut *ctx.executor,
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
        let new_wo_id = self.work_order.create(
            ctx.reborrow(),
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
            self.work_order.find_by_id(ctx.reborrow(), orig_wo_id).await.ok()
        } else {
            None
        };
        let return_warehouse_id = wo.and_then(|w| w.work_center_id).unwrap_or(0);

        // WMS: 材料回调 — 创建调拨单、发货、完成
        let materials = OutsourcingMaterialRepo::list_by_outsourcing_id(&mut *ctx.executor, req.id)
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
            let tid = self.transfer_service
                .create(
                    ctx.reborrow(),
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
            self.transfer_service.dispatch(ctx.reborrow(), tid).await?;
            self.transfer_service.complete(ctx.reborrow(), tid).await?;
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
            self.doc_link.create_links(ctx.reborrow(), convert_links).await?;
        }

        // 审计
        self.audit_log
            .record(
                ctx.reborrow(),
                ENTITY_TYPE,
                req.id,
                AuditAction::Transition,
                Some(json!({ "from": format!("{:?}", order.status), "to": "ConvertedToInternal", "new_work_order_id": new_wo_id })),
                None,
            )
            .await?;

        // 领域事件: OutsourcingConvertedToInternal
        let remaining_materials: Vec<serde_json::Value> = materials
            .iter()
            .filter(|m| m.in_transit_qty() > Decimal::ZERO)
            .map(|m| json!({
                "material_id": m.id,
                "product_id": m.product_id,
                "remaining_qty": m.in_transit_qty().to_string(),
            }))
            .collect();
        self.event_bus
            .publish(
                ctx,
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
        mut ctx: ServiceContext<'_>,
        req: CancelOutsourcingReq,
    ) -> Result<(), DomainError> {
        let order = get_order(&mut ctx.reborrow(), req.id).await?;
        if order.status != OutsourcingStatus::Draft {
            return Err(DomainError::validation("仅 DRAFT 状态可取消"));
        }
        check_version(&order, req.expected_version)?;

        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, req.id, "Cancelled", req.remark.as_deref())
            .await?;

        let rows = OutsourcingOrderRepo::update_status_and_version(
            &mut *ctx.executor,
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
        self.audit_log
            .record(
                ctx.reborrow(),
                ENTITY_TYPE,
                req.id,
                AuditAction::Transition,
                Some(json!({ "from": "Draft", "to": "Cancelled" })),
                None,
            )
            .await?;

        // 领域事件
        self.event_bus
            .publish(
                ctx,
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
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<OutsourcingOrder, DomainError> {
        get_order(&mut ctx.reborrow(), id).await
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: OutsourcingOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<OutsourcingOrder>, DomainError> {
        let scope = (ctx.data_scope, ctx.operator_id, ctx.department_id);
        let (items, total) = OutsourcingOrderRepo::query(&mut *ctx.executor, &filter, &page, scope)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}
