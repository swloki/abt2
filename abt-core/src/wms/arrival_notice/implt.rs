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
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::ArrivalStatus;
use crate::wms::stubs::{
    CostEntryStub, CostEntryReq, DocumentLinkStub, DocumentSequenceStub,
    InventoryReservationStub, QualityGateStub,
};

pub struct ArrivalNoticeServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl ArrivalNoticeServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
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

        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "AN-")
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
            let _ = DocumentLinkStub::link(
                ctx.reborrow(),
                "arrival_notice",
                notice.id,
                "purchase_order",
                po_id,
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

        // IQC 硬门禁：不合格时阻止 Accepted/PartiallyAccepted，强制降为 Rejected
        // 设计要求：confirm requires QMS.InspectionResultService.is_passed(IQC) hard gate
        let quality_passed = QualityGateStub::is_passed(ctx.reborrow(), "arrival_notice", req.id)
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
            let _ = CostEntryStub::record(
                ctx.reborrow(),
                CostEntryReq {
                    cost_type: "material_cost".to_string(),
                    debit_account: "inventory".to_string(),
                    credit_account: "accounts_payable".to_string(),
                    amount: items.iter().map(|i| i.accepted_qty).fold(Decimal::ZERO, |a, b| a + b),
                    source_type: "arrival_notice".to_string(),
                    source_id: req.id,
                },
            )
            .await;

            // confirm -> InvRes(release safety stock)
            let _ = InventoryReservationStub::release(
                ctx.reborrow(),
                "arrival_notice",
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
