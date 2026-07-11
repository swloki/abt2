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
use crate::fms::ar_ap::repo::ArApLedgerRepo;
use crate::fms::enums::CounterpartyType;
use crate::purchase::return_order::repo::{PurchaseReturnItemRepo, PurchaseReturnRepo};
use crate::shared::enums::DocumentType;
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result, ServiceContext};

/// 采购退货结算 Handler
///
/// 监听 `PurchaseReturnSettled` 事件：退货单经对账单结算后，写反向 AP 台账（`Debit`）
/// 冲减应付。幂等 + 往来方币种由 `ArApLedgerRepo::insert_reversal_if_absent` 统一处理。
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

        // 1. 取退货单主表 + 明细（域特定）
        let ret = PurchaseReturnRepo::get_by_id(&mut conn, return_id)
            .await?
            .ok_or_else(|| DomainError::not_found(format!("PurchaseReturn #{return_id}")))?;
        let items = PurchaseReturnItemRepo::list_by_return_id(&mut conn, return_id).await?;

        // 退货冲减金额 = Σ 明细 amount
        let refund_amount: Decimal = items.iter().map(|i| i.amount).sum();
        if refund_amount <= Decimal::ZERO {
            warn!(return_id, "PurchaseReturn refund amount <= 0, skipping AP reversal");
            return Ok(());
        }

        // 2. 公共：幂等 + 按往来方币种 insert 反向 AP 台账（Debit 冲减 Credit）
        let desc = format!("采购退货冲减应付 {}", ret.doc_number);
        let inserted = ArApLedgerRepo::insert_reversal_if_absent(
            &mut conn,
            CounterpartyType::Supplier,
            ret.supplier_id,
            DocumentType::PurchaseReturn,
            return_id,
            &ret.doc_number,
            LedgerDirection::Debit,
            refund_amount,
            &desc,
            ctx.operator_id,
        )
        .await?;

        if inserted.is_some() {
            info!(
                return_id,
                supplier_id = ret.supplier_id,
                amount = %refund_amount,
                "AP ledger Debit (purchase return reversal) inserted"
            );
        } else {
            info!(return_id, "PurchaseReturn already has AP ledger entry, skipping");
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "purchase_return_settled"
    }
}
