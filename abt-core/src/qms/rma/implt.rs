use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo;
use super::service::RmaService;
use crate::qms::enums::*;
use crate::qms::inspection_result;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::service::DocumentLinkService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "RMA";

pub struct RmaServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    doc_link: Arc<dyn DocumentLinkService>,
}

impl RmaServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
        doc_link: Arc<dyn DocumentLinkService>,
    ) -> Self {
        Self { pool, doc_seq, state_machine, event_bus, audit_log, doc_link }
    }
}

#[async_trait]
impl RmaService for RmaServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateRmaReq,
    ) -> Result<i64> {
        // 1. 校验关联检验结果（如有）
        if let Some(ir_id) = req.linked_inspection_result_id {
            let ir = inspection_result::repo::find_by_id(&mut *ctx.executor, ir_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            if ir.is_none() {
                return Err(DomainError::validation(format!(
                    "关联检验结果 {} 不存在",
                    ir_id
                )));
            }
        }

        // 2. 生成单据编号
        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::Rma)
            .await?;

        // 3. 构建实体并插入
        let now = chrono::Utc::now();
        let rma = Rma {
            id: 0,
            doc_number,
            customer_id: req.customer_id,
            sales_order_id: req.sales_order_id,
            shipping_request_id: req.shipping_request_id,
            product_id: req.product_id,
            linked_inspection_result_id: req.linked_inspection_result_id,
            defect_description: req.defect_description,
            severity: req.severity,
            root_cause: None,
            corrective_action: None,
            status: RMAStatus::Reported,
            remark: req.remark,
            operator_id: ctx.operator_id,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = repo::insert(&mut *ctx.executor, &rma)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 发布 RMACreated 事件
        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::RMACreated,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({
                        "doc_number": rma.doc_number,
                        "customer_id": req.customer_id,
                        "product_id": req.product_id,
                        "severity": req.severity.as_i16(),
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 5. 审计日志
        self.audit_log
            .record(ctx.reborrow(), ENTITY_TYPE, id, AuditAction::Create, None, None)
            .await?;

        // 6. 构建 DocumentLink: RMA → InspectionResult（正向→逆向追溯链）
        if let Some(ir_id) = req.linked_inspection_result_id {
            self.doc_link
                .create_links(
                    ctx.reborrow(),
                    vec![LinkRequest {
                        source_type: DocumentType::Rma,
                        source_id: id,
                        target_type: DocumentType::InspectionResult,
                        target_id: ir_id,
                        link_type: LinkType::References,
                    }],
                )
                .await?;
        }

        Ok(id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Rma> {
        repo::find_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    /// 记录根因 — 自动触发 Reported → Investigating → ActionTaken
    async fn record_root_cause(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        req: RecordRootCauseReq,
    ) -> Result<()> {
        // 1. 获取当前状态并校验
        let existing = repo::find_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        if existing.status != RMAStatus::Reported && existing.status != RMAStatus::Investigating {
            return Err(DomainError::validation(format!(
                "RMA 状态为 {:?}，只有 Reported 或 Investigating 才能记录根因",
                existing.status
            )));
        }

        // 2. 自动触发状态转换: Reported → Investigating（仅 Reported 起始）
        if existing.status == RMAStatus::Reported {
            self.state_machine
                .transition(ctx.reborrow(), ENTITY_TYPE, id, "Investigating", None)
                .await?;
            let rows = repo::update_status(
                &mut *ctx.executor,
                id,
                RMAStatus::Investigating.as_i16(),
                RMAStatus::Reported.as_i16(),
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if rows == 0 {
                return Err(DomainError::ConcurrentConflict);
            }
        }

        // 3. 写入根因和纠正措施（在状态验证通过后）
        let rows = repo::update_root_cause(
            &mut *ctx.executor,
            id,
            &req.root_cause,
            &req.corrective_action,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 4. 推进到 ActionTaken
        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, id, "ActionTaken", None)
            .await?;
        let rows = repo::update_status(
            &mut *ctx.executor,
            id,
            RMAStatus::ActionTaken.as_i16(),
            RMAStatus::Investigating.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 5. 审计日志
        self.audit_log
            .record(ctx, ENTITY_TYPE, id, AuditAction::Transition, None, None)
            .await?;

        Ok(())
    }

    async fn close(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()> {
        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, id, "Closed", None)
            .await?;

        let rows = repo::update_status(
            &mut *ctx.executor,
            id,
            RMAStatus::Closed.as_i16(),
            RMAStatus::ActionTaken.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 构建 DocumentLink: RMA → SalesOrder / ShippingRequest（追溯链）
        let rma = repo::find_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        let mut links: Vec<LinkRequest> = Vec::new();
        if let Some(so_id) = rma.sales_order_id {
            links.push(LinkRequest {
                source_type: DocumentType::Rma,
                source_id: id,
                target_type: DocumentType::SalesOrder,
                target_id: so_id,
                link_type: LinkType::References,
            });
        }
        if let Some(sr_id) = rma.shipping_request_id {
            links.push(LinkRequest {
                source_type: DocumentType::Rma,
                source_id: id,
                target_type: DocumentType::ShippingRequest,
                target_id: sr_id,
                link_type: LinkType::References,
            });
        }
        if !links.is_empty() {
            self.doc_link.create_links(ctx.reborrow(), links).await?;
        }

        self.audit_log
            .record(ctx, ENTITY_TYPE, id, AuditAction::Transition, None, None)
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: RmaFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<Rma>> {
        repo::list(&mut *ctx.executor, &filter, &page)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
