use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::fms::adjustment::enums::AdjustmentDirection;
use crate::fms::adjustment::model::*;
use crate::fms::adjustment::repo::AdjustmentRepo;
use crate::fms::adjustment::service::AdjustmentService;
use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::enums::CounterpartyType;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, model::EventPublishRequest, service::DomainEventBus};
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

pub struct AdjustmentServiceImpl {
    pool: PgPool,
}

impl AdjustmentServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl AdjustmentService for AdjustmentServiceImpl {
    async fn create_adjustment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateAdjustmentReq,
    ) -> Result<i64> {
        if req.amount <= Decimal::ZERO {
            return Err(DomainError::validation("adjustment amount must be greater than zero"));
        }
        // 多币种校验（issue #69）：汇率必须 > 0；CNY 强制汇率 = 1
        if req.exchange_rate <= Decimal::ZERO {
            return Err(DomainError::validation("exchange rate must be greater than zero"));
        }
        if req.currency.eq_ignore_ascii_case("CNY") && req.exchange_rate != Decimal::ONE {
            return Err(DomainError::validation("CNY exchange rate must be 1"));
        }
        match req.party_type {
            CounterpartyType::Customer | CounterpartyType::Supplier => {}
            _ => {
                return Err(DomainError::validation(
                    "adjustment only supports Customer or Supplier",
                ))
            }
        }

        // 1. 生成单号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ArApAdjustment)
            .await?;

        // 2. 插入调整单（币种与汇率取自请求，issue #69）
        let id = AdjustmentRepo::create(
            db,
            &doc_number,
            &req,
            &req.currency,
            req.exchange_rate,
            ctx.operator_id,
        )
        .await?;

        // 3. 业务方向 → 台账 LedgerDirection（与 cash_journal 一致）
        let ledger_dir = match (req.party_type, req.direction) {
            (CounterpartyType::Customer, AdjustmentDirection::Increase) => LedgerDirection::Debit,
            (CounterpartyType::Customer, AdjustmentDirection::Decrease) => LedgerDirection::Credit,
            (CounterpartyType::Supplier, AdjustmentDirection::Increase) => LedgerDirection::Credit,
            (CounterpartyType::Supplier, AdjustmentDirection::Decrease) => LedgerDirection::Debit,
            _ => unreachable!(),
        };

        let dir_label = match req.direction {
            AdjustmentDirection::Increase => "增加",
            AdjustmentDirection::Decrease => "减少",
        };
        let desc = if req.description.is_empty() {
            format!("应收应付调整 {doc_number} {dir_label}")
        } else {
            format!("应收应付调整 {doc_number} {dir_label} — {}", req.description)
        };

        // 4. 写台账（同事务）— 币种与汇率取自请求（issue #69）
        let ledger_id = ArApLedgerRepo::insert(
            db,
            &ArApLedgerInsert {
                party_type: req.party_type,
                party_id: req.party_id,
                source_type: DocumentType::ArApAdjustment,
                source_id: id,
                source_doc_no: &doc_number,
                against_type: None,
                against_id: None,
                direction: ledger_dir,
                amount: req.amount,
                currency: &req.currency,
                exchange_rate: req.exchange_rate,
                transaction_date: req.adjustment_date,
                due_date: None,
                period: &req.period,
                description: &desc,
                operator_id: ctx.operator_id,
            },
        )
        .await?
        .ok_or_else(|| DomainError::business_rule("ar_ap ledger insert conflicted"))?;

        // 6. 回填 ledger_id
        AdjustmentRepo::update_ledger_id(db, id, ledger_id).await?;

        // 7. 审计
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ArApAdjustment",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: Some(serde_json::json!({
                        "doc_number": doc_number,
                        "party_type": req.party_type.as_i16(),
                        "party_id": req.party_id,
                        "direction": req.direction.as_i16(),
                        "amount": req.amount,
                        "ledger_id": ledger_id,
                    })),
                    context: None,
                },
            )
            .await?;

        // 8. 发事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::ArApAdjustmentPosted,
                    aggregate_type: "ArApAdjustment".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "adjustment_id": id,
                        "doc_number": doc_number,
                        "party_type": req.party_type.as_i16(),
                        "party_id": req.party_id,
                        "direction": req.direction.as_i16(),
                        "amount": req.amount,
                        "ledger_id": ledger_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn get_adjustment(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ArApAdjustment> {
        AdjustmentRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ArApAdjustment"))
    }

    async fn list_adjustments(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: AdjustmentFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<AdjustmentRow>> {
        let (items, total) = AdjustmentRepo::query(db, &filter, &page).await?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}
