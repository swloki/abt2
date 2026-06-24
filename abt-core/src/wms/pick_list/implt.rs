use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::{PickListItemRepo, PickListRepo};
use super::service::PickListService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};
use crate::wms::outbound::{new_shipping_request_service, service::ShippingRequestService};

pub struct PickListServiceImpl {
    repo: PickListRepo,
    item_repo: PickListItemRepo,
    pool: PgPool,
}

impl PickListServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: PickListRepo,
            item_repo: PickListItemRepo,
            pool,
        }
    }
}

#[async_trait]
impl PickListService for PickListServiceImpl {
    async fn generate_from_outbound(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outbound_id: i64,
    ) -> Result<i64> {
        // 幂等：已有拣货单则直接返回
        if let Some(existing) = self.repo.find_by_outbound(db, outbound_id).await? {
            return Ok(existing.id);
        }

        // 经 ShippingRequestService trait 校验发货单存在 + 取明细（禁止 repo 直访）
        new_shipping_request_service(self.pool.clone())
            .find_by_id(ctx, db, outbound_id)
            .await?;
        let items = new_shipping_request_service(self.pool.clone())
            .list_items(ctx, db, outbound_id)
            .await?;

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PickList)
            .await?;

        let id = self.repo.insert(
            db,
            &CreatePickListParams {
                doc_number: &doc_number,
                outbound_id,
                picker_id: None,
                remark: "",
                operator_id: ctx.operator_id,
            },
        )
        .await?;

        // 明细：MVP picked_qty = requested_qty 自动满拣（前端后续支持人工调整/部分拣）
        let item_inputs: Vec<PickListItemInput> = items
            .iter()
            .enumerate()
            .map(|(i, it)| PickListItemInput {
                line_no: (i + 1) as i32,
                outbound_item_id: it.id,
                product_id: it.product_id,
                warehouse_id: it.warehouse_id,
                bin_id: None,
                requested_qty: it.requested_qty,
                picked_qty: it.requested_qty,
            })
            .collect();
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "PickListStatus", id, "Draft", None)
            .await?;

        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::PickList,
                    source_id: id,
                    target_type: DocumentType::ShippingRequest,
                    target_id: outbound_id,
                    link_type: LinkType::Triggers,
                }],
            )
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "PickList",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: Some(serde_json::json!({ "outbound_id": outbound_id, "doc_number": doc_number })),
                    context: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn complete_pick(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("PickList"))?;
        if existing.status != PickListStatus::Draft {
            return Err(DomainError::business_rule("只有 Draft 状态的拣货单可完成"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "PickListStatus", id, "Picked", None)
            .await?;
        self.repo.mark_picked(db, id, None).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "PickList",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({ "from": "Draft", "to": "Picked" })),
                    context: None,
                },
            )
            .await?;
        Ok(())
    }

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("PickList"))?;
        if existing.status != PickListStatus::Draft {
            return Err(DomainError::business_rule("只有 Draft 状态的拣货单可取消"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "PickListStatus", id, "Cancelled", None)
            .await?;
        self.repo.update_status(db, id, PickListStatus::Cancelled).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "PickList",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({ "from": "Draft", "to": "Cancelled" })),
                    context: None,
                },
            )
            .await?;
        Ok(())
    }

    async fn find_by_id(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PickList> {
        self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("PickList"))
    }

    async fn find_by_outbound(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outbound_id: i64,
    ) -> Result<Option<PickList>> {
        self.repo.find_by_outbound(db, outbound_id).await
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        pick_list_id: i64,
    ) -> Result<Vec<PickListItem>> {
        self.item_repo
            .find_by_pick_list_id(db, pick_list_id)
            .await
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PickListQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PickList>> {
        self.repo.list(db, &filter, &page).await
    }
}
