use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{
    ArrivalNotice, ArrivalNoticeFilter, CreateArrivalNoticeReq, InspectArrivalNoticeReq,
    ReceiveArrivalNoticeReq,
};
use super::repo::ArrivalNoticeRepo;
use super::service::ArrivalNoticeService;
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
use crate::qms::inspection_result::model::InspectionResultFilter;
use crate::qms::inspection_result::service::InspectionResultService;
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::service::CostEntryService;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::service::DocumentLinkService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::{CostEntityType, CostType, DocumentType, LinkType};
use crate::shared::inventory_reservation::service::InventoryReservationService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::ArrivalStatus;

pub struct ArrivalNoticeServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    doc_link: Arc<dyn DocumentLinkService>,
    cost_entry: Arc<dyn CostEntryService>,
    inv_res: Arc<dyn InventoryReservationService>,
    qms: Arc<dyn InspectionResultService>,
}

impl ArrivalNoticeServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        doc_link: Arc<dyn DocumentLinkService>,
        cost_entry: Arc<dyn CostEntryService>,
        inv_res: Arc<dyn InventoryReservationService>,
        qms: Arc<dyn InspectionResultService>,
    ) -> Self {
        Self { pool, doc_seq, doc_link, cost_entry, inv_res, qms }
    }
}

#[async_trait]
impl ArrivalNoticeService for ArrivalNoticeServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateArrivalNoticeReq,
    ) -> Result<i64, DomainError> {
        if req.items.is_empty() {
            return Err(DomainError::validation("来料通知必须包含至少一条明细"));
        }

        let doc_number = self.doc_seq.next_number(ctx.reborrow(), DocumentType::ArrivalNotice)
            .await
            .unwrap_or_else(|_| generate_doc_number_fallback());

        let notice = ArrivalNoticeRepo::insert(
            &mut *ctx.executor,
            &doc_number,
            &req,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // DocumentLink: 来料 → 采购单
        if let Some(po_id) = req.purchase_order_id {
            let _ = self.doc_link.create_links(
                ctx.reborrow(),
                vec![LinkRequest {
                    source_type: DocumentType::ArrivalNotice,
                    source_id: notice.id,
                    target_type: DocumentType::PurchaseOrder,
                    target_id: po_id,
                    link_type: LinkType::Fulfills,
                }],
            )
            .await;
        }

        Ok(notice.id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ArrivalNotice, DomainError> {
        ArrivalNoticeRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("ArrivalNotice #{id}")))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ArrivalNoticeFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ArrivalNotice>, DomainError> {
        ArrivalNoticeRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn receive(
        &self,
        ctx: ServiceContext<'_>,
        req: ReceiveArrivalNoticeReq,
    ) -> Result<(), DomainError> {
        let notice = ArrivalNoticeRepo::get_by_id(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("ArrivalNotice #{}", req.id)))?;

        if notice.status != ArrivalStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", notice.status),
                to: "Received".to_string(),
            });
        }

        for item in &req.items {
            let affected =
                ArrivalNoticeRepo::update_item_received(&mut *ctx.executor, item.item_id, item.received_qty, item.batch_no.as_deref())
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;

            if affected == 0 {
                return Err(DomainError::not_found(format!(
                    "ArrivalNoticeItem #{}",
                    item.item_id
                )));
            }
        }

        let affected = ArrivalNoticeRepo::update_status(
            &mut *ctx.executor,
            req.id,
            ArrivalStatus::Received,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!(
                "ArrivalNotice #{}",
                req.id
            )));
        }

        Ok(())
    }

    async fn inspect(
        &self,
        mut ctx: ServiceContext<'_>,
        req: InspectArrivalNoticeReq,
    ) -> Result<(), DomainError> {
        let notice = ArrivalNoticeRepo::get_by_id(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("ArrivalNotice #{}", req.id)))?;

        if notice.status != ArrivalStatus::Received {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", notice.status),
                to: "Inspecting".to_string(),
            });
        }

        // Received -> Inspecting
        ArrivalNoticeRepo::update_status(
            &mut *ctx.executor,
            req.id,
            ArrivalStatus::Inspecting,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 更新每条明细的 accepted_qty
        for item in &req.items {
            let affected = ArrivalNoticeRepo::update_item_accepted(
                &mut *ctx.executor,
                item.item_id,
                item.accepted_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            if affected == 0 {
                return Err(DomainError::not_found(format!(
                    "ArrivalNoticeItem #{}",
                    item.item_id
                )));
            }
        }

        // 查询更新后的所有明细，判定最终状态
        let items = ArrivalNoticeRepo::get_items(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let preliminary_status = determine_inspection_result(&items);

        // IQC 硬门禁：查询 QMS 检验结果，判定是否通过
        let quality_passed = check_qms_gate(
            &self.qms,
            ctx.reborrow(),
            InspectionSourceType::ArrivalNotice,
            req.id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let final_status = if !quality_passed
            && (preliminary_status == ArrivalStatus::Accepted
                || preliminary_status == ArrivalStatus::PartiallyAccepted)
        {
            ArrivalStatus::Rejected
        } else {
            preliminary_status
        };

        // confirm -> CostEntry(材料成本: debit inventory / credit AP) [IndependentTx]
        if final_status == ArrivalStatus::Accepted || final_status == ArrivalStatus::PartiallyAccepted {
            let total_accepted = items.iter().map(|i| i.accepted_qty).fold(Decimal::ZERO, |a, b| a + b);
            let period = chrono::Local::now().format("%Y-%m").to_string();

            let _ = self.cost_entry.create_entries(
                ctx.reborrow(),
                vec![EntryRequest {
                    entity_type: CostEntityType::PurchaseOrder,
                    entity_id: req.id,
                    cost_type: CostType::Material,
                    debit_amount: total_accepted,
                    credit_amount: total_accepted,
                    cost_center: None,
                    profit_center: None,
                    period,
                    source_type: DocumentType::ArrivalNotice,
                    source_id: req.id,
                }],
            )
            .await;

            // confirm -> InvRes(release safety stock)
            let _ = self.inv_res.cancel_by_source(
                ctx.reborrow(),
                DocumentType::ArrivalNotice,
                req.id,
            )
            .await;
        }

        ArrivalNoticeRepo::update_status(&mut *ctx.executor, req.id, final_status)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn cancel(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let notice = ArrivalNoticeRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("ArrivalNotice #{id}")))?;

        if notice.status != ArrivalStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", notice.status),
                to: "Cancelled".to_string(),
            });
        }

        let affected = ArrivalNoticeRepo::soft_delete(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("ArrivalNotice #{id}")));
        }

        Ok(())
    }
}

/// 检查 QMS 质量关卡：查询 source 关联的检验结果，判断是否全部通过
async fn check_qms_gate(
    qms: &Arc<dyn InspectionResultService>,
    ctx: ServiceContext<'_>,
    source_type: InspectionSourceType,
    source_id: i64,
) -> Result<bool, DomainError> {
    let results = qms.list_by_source(
        ctx,
        InspectionResultFilter {
            source_type: Some(source_type),
            source_id: Some(source_id),
            ..Default::default()
        },
        crate::shared::types::pagination::PageParams { page: 1, page_size: 100 },
    )
    .await?;

    if results.items.is_empty() {
        return Ok(true);
    }

    Ok(results.items.iter().all(|r| {
        r.status == InspectionStatus::Completed && r.result == InspectionResultType::Pass
    }))
}

/// 根据明细行检验结果判定最终状态
fn determine_inspection_result(items: &[super::model::ArrivalNoticeItem]) -> ArrivalStatus {
    if items.is_empty() {
        return ArrivalStatus::Accepted;
    }

    let all_accepted = items
        .iter()
        .all(|i| i.accepted_qty == i.received_qty);
    let all_rejected = items
        .iter()
        .all(|i| i.accepted_qty == Decimal::ZERO);

    if all_accepted {
        ArrivalStatus::Accepted
    } else if all_rejected {
        ArrivalStatus::Rejected
    } else {
        ArrivalStatus::PartiallyAccepted
    }
}

fn generate_doc_number_fallback() -> String {
    let now = chrono::Utc::now();
    format!("AN-{}-{:04}", now.format("%Y%m%d%H%M%S"), (now.timestamp_nanos_opt().unwrap_or(0) as u32) % 10000)
}
