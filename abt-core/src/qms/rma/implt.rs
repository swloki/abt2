use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo;
use super::service::RmaService;
use crate::qms::enums::*;
use crate::qms::inspection_result;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::types::PgExecutor;
use crate::shared::document_link::{new_document_link_service, model::LinkRequest, service::DocumentLinkService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::{new_domain_event_bus, model::EventPublishRequest, service::DomainEventBus};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "RMA";

pub struct RmaServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl RmaServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RmaService for RmaServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateRmaReq,
    ) -> Result<i64> {
        // 1. 校验关联检验结果（如有）
        if let Some(ir_id) = req.linked_inspection_result_id {
            let ir = inspection_result::repo::find_by_id(&mut *db, ir_id)
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
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::Rma)
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

        let id = repo::insert(&mut *db, &rma)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 发布 RMACreated 事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
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
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        // 6. 构建 DocumentLink: RMA → InspectionResult（正向→逆向追溯链）
        if let Some(ir_id) = req.linked_inspection_result_id {
            new_document_link_service(self.pool.clone())
                .create_links(
                    ctx,
                    db,
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
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Rma> {
        repo::find_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    /// 记录根因 — 自动触发 Reported → Investigating → ActionTaken
    async fn record_root_cause(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: RecordRootCauseReq,
    ) -> Result<()> {
        // 1. 获取当前状态并校验
        let existing = repo::find_by_id(&mut *db, id)
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
            new_state_machine_service(self.pool.clone())
                .transition(ctx, db, ENTITY_TYPE, id, "Investigating", None)
                .await?;
            let rows = repo::update_status(
                &mut *db,
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
            &mut *db,
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
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "ActionTaken", None)
            .await?;
        let rows = repo::update_status(
            &mut *db,
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
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn close(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Closed", None)
            .await?;

        let rows = repo::update_status(
            &mut *db,
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
        let rma = repo::find_by_id(&mut *db, id)
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
            new_document_link_service(self.pool.clone())
                .create_links(ctx, db, links).await?;
        }

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: RmaFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<Rma>> {
        repo::list(&mut *db, &filter, &page)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
