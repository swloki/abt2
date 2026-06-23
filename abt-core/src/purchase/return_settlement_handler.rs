//! 采购退货结算 Handler — 监听 `PurchaseReturnSettled` 事件，写反向 AP 台账冲减应付。
//!
//! 业务背景：采购对账单 `confirm()` 在结算关联退货单（`Shipped → Settled`）时发布
//! `PurchaseReturnSettled` 事件（见 `reconciliation/implt.rs`）。本 handler 消费该事件，
//! 写一笔反向 AP 台账（`Debit`，应付减少），冲减入库时由 `ArrivalAcceptedHandler` 立的 `Credit`。
//! 对齐 ERPNext / Odoo / OFBiz「退货以反向单据（credit note）冲减应付」的共识（Issue #85）。

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use tracing::{info, warn};

use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::enums::CounterpartyType;
use crate::purchase::return_order::repo::{PurchaseReturnItemRepo, PurchaseReturnRepo};
use crate::shared::enums::DocumentType;
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result, ServiceContext};

/// 采购退货结算 Handler
///
/// 监听 `PurchaseReturnSettled` 事件：退货单经对账单结算后，写反向 AP 台账（`Debit`）
/// 冲减应付。幂等：同一退货单（`source_type = PurchaseReturn, source_id = return_id`）不重复立账。
pub struct PurchaseReturnSettledHandler {
    pool: PgPool,
}

impl PurchaseReturnSettledHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for PurchaseReturnSettledHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let return_id = event.aggregate_id;

        let ctx = ServiceContext::system();
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 1. 幂等：同一退货单不重复写台账（事件重放 / 重复结算保护）
        let dup: Option<i64> = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT id FROM ar_ap_ledger WHERE source_type = $1 AND source_id = $2 LIMIT 1",
        )
        .bind(DocumentType::PurchaseReturn)
        .bind(return_id)
        .fetch_optional(&mut *conn)
        .await?;

        if dup.is_some() {
            info!(return_id, "PurchaseReturn already has AP ledger entry, skipping");
            return Ok(());
        }

        // 2. 取退货单主表 + 明细
        let ret = PurchaseReturnRepo::get_by_id(&mut conn, return_id)
            .await?
            .ok_or_else(|| DomainError::not_found(format!("PurchaseReturn #{return_id}")))?;

        let items = PurchaseReturnItemRepo::list_by_return_id(&mut conn, return_id).await?;

        // 退货冲减金额 = Σ 明细 amount（以明细为准，与主表 total_amount 口径一致）
        let refund_amount: Decimal = items.iter().map(|i| i.amount).sum();

        if refund_amount <= Decimal::ZERO {
            warn!(return_id, "PurchaseReturn refund amount <= 0, skipping AP reversal");
            return Ok(());
        }

        // 3. 写反向 AP 台账（Debit 冲减入库时立的 Credit）
        let period = chrono::Utc::now().format("%Y-%m").to_string();
        let today = chrono::Local::now().date_naive();
        let desc = format!("采购退货冲减应付 {}", ret.doc_number);

        ArApLedgerRepo::insert(
            &mut *conn,
            &ArApLedgerInsert {
                party_type: CounterpartyType::Supplier,
                party_id: ret.supplier_id,
                source_type: DocumentType::PurchaseReturn,
                source_id: return_id,
                source_doc_no: &ret.doc_number,
                against_type: None,
                against_id: None,
                direction: LedgerDirection::Debit,
                amount: refund_amount,
                currency: "CNY",
                exchange_rate: Decimal::ONE,
                transaction_date: today,
                due_date: None,
                period: &period,
                description: &desc,
                operator_id: ctx.operator_id,
            },
        )
        .await?;

        info!(
            return_id,
            supplier_id = ret.supplier_id,
            amount = %refund_amount,
            "AP ledger Debit (purchase return reversal) inserted"
        );

        Ok(())
    }

    fn name(&self) -> &str {
        "purchase_return_settled"
    }
}
