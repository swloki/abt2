use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;
use chrono::NaiveDate;

use super::model::{PurchaseReconciliation, PurchaseReconPreviewItem, PurchaseReconciliationQuery};
use super::repo::{NewReconItem, PurchaseReconItemRepo, PurchaseReconciliationRepo};
use super::service::PurchaseReconciliationService;
use crate::purchase::enums::{InvoiceStatus, PurchaseOrderStatus, PurchaseReconStatus, PurchaseReturnStatus};
use crate::purchase::order::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
use crate::purchase::return_order::repo::PurchaseReturnRepo;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::idempotency::{new_idempotency_service, service::{key_to_i64, IdempotencyService}};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::PgExecutor;
use crate::shared::types::PageParams;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

const ENTITY_TYPE: &str = "PurchaseReconciliation";
const ENTITY_DISPLAY: &str = "采购对账单";

pub struct PurchaseReconciliationServiceImpl {
    pool: PgPool,
}

impl PurchaseReconciliationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseReconciliationService for PurchaseReconciliationServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        supplier_id: i64,
        period: String,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseReconciliation:create").await? {
                return Err(DomainError::duplicate("PurchaseReconciliation"));
            }
        }
        // 1. 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PurchaseReconciliation)
            .await?;

        // 2. 解析期间字符串（格式 YYYY-MM），查询该供应商当期未对账的已收货订单明细
        let (period_start, period_end) = parse_period(&period)?;

        let order_items = PurchaseOrderItemRepo::list_unreconciled_received_by_supplier(
            &mut *db,
            supplier_id,
            period_start,
            period_end,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 3. 构建对账明细
        let recon_items: Vec<NewReconItem> = order_items
            .iter()
            .map(|item| {
                let amount = item.received_qty * item.unit_price;
                NewReconItem {
                    order_id: item.order_id,
                    order_item_id: item.id,
                    received_qty: item.received_qty,
                    returned_qty: item.returned_qty,
                    returned_amount: item.returned_qty * item.unit_price,
                    unit_price: item.unit_price,
                    amount,
                }
            })
            .collect();

        // 4. 计算对账总金额
        let total_amount: Decimal = recon_items.iter().map(|i| i.amount).sum();

        // 5. 插入主表
        let id = PurchaseReconciliationRepo::insert(
            &mut *db,
            supplier_id,
            &period,
            total_amount,
            &doc_number,
            "",
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 6. 插入对账明细
        if !recon_items.is_empty() {
            PurchaseReconItemRepo::insert_items(&mut *db, id, &recon_items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 7. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: ENTITY_TYPE,
                        entity_id: id,
                        action: AuditAction::Create,
                        changes: None,
                        context: None,
                    },
                )
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
            .await?;

        Ok(id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<PurchaseReconciliation> {
        PurchaseReconciliationRepo::get_by_id(db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: PurchaseReconciliationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseReconciliation>> {
        let (items, total) = PurchaseReconciliationRepo::query(&mut *db, &query, &page)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        reconciliation_id: i64,
    ) -> Result<Vec<super::model::PurchaseReconItem>> {
        PurchaseReconItemRepo::list_by_reconciliation_id(&mut *db, reconciliation_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseReconciliation:confirm").await? {
                return Err(DomainError::duplicate("PurchaseReconciliation"));
            }
        }
        // 1. 获取对账单及明细
        let recon = PurchaseReconciliationRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        let recon_items = PurchaseReconItemRepo::list_by_reconciliation_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 2. 计算确认金额和差异
        let confirmed_amount: Decimal = recon_items
            .iter()
            .map(|i| i.amount - i.returned_amount)
            .sum();
        let difference = recon.total_amount - confirmed_amount;

        // 3. 写回确认金额和差异到主表
        let rows = PurchaseReconciliationRepo::update_confirmed_amount(
            &mut *db,
            id,
            confirmed_amount,
            difference,
            &recon.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 3.1 重新读取以获取 update_confirmed_amount 设置的 updated_at = NOW()
        let recon = PurchaseReconciliationRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        // 4. 更新确认标识（逐行标记 confirmed = true）
        for item in &recon_items {
            PurchaseReconItemRepo::confirm_item(&mut *db, item.id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 5. 驱动关联退货单状态：Shipped -> Settled（仅限本对账单涉及的订单）
        let mut order_ids: Vec<i64> = recon_items.iter().map(|i| i.order_id).collect();
        order_ids.sort();
        order_ids.dedup();

        // 4.6 更新 PO 明细的 qty_invoiced 并重算开票状态
        for item in &recon_items {
            PurchaseOrderItemRepo::add_qty_invoiced(&mut *db, item.order_item_id, item.received_qty)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        for &po_id in &order_ids {
            let po_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, po_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            let all_invoiced = po_items.iter().all(|i| {
                i.qty_invoiced >= i.received_qty && i.qty_invoiced > Decimal::ZERO
            });
            let any_invoiced = po_items.iter().any(|i| i.qty_invoiced > Decimal::ZERO);
            let po_status = if all_invoiced {
                InvoiceStatus::FullyInvoiced
            } else if any_invoiced {
                InvoiceStatus::ToInvoice
            } else {
                InvoiceStatus::NoInvoice
            };
            let total: Decimal = po_items.iter().map(|i| i.price_total).sum();
            let invoiced: Decimal =
                po_items.iter().map(|i| i.qty_invoiced * i.unit_price).sum();
            let per_billed = if total > Decimal::ZERO {
                invoiced / total * Decimal::from(100)
            } else {
                Decimal::ZERO
            };
            PurchaseOrderRepo::update_invoice_status(&mut *db, po_id, po_status, per_billed)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

            // 5.0 自动闭环：全部收货 + 全部开票 = 自动关闭 PO
            if po_status == InvoiceStatus::FullyInvoiced
                && let Some(po) = PurchaseOrderRepo::get_by_id(&mut *db, po_id).await?
                    && po.status == PurchaseOrderStatus::Received {
                        new_state_machine_service(self.pool.clone())
                            .transition(ctx, db, "PurchaseOrder", po_id, "Closed", Some("对账完成自动关闭"))
                            .await?;

                        let _ = PurchaseOrderRepo::update_status(
                            &mut *db, po_id,
                            PurchaseOrderStatus::Closed,
                            &po.updated_at,
                        ).await;
                    }
        }

        let returns = PurchaseReturnRepo::list_shipped_by_supplier_for_orders(
            &mut *db,
            recon.supplier_id,
            &order_ids,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        for ret in &returns {
            new_state_machine_service(self.pool.clone())
                .transition(
                    ctx, db,
                    "PurchaseReturn",
                    ret.id,
                    "Settled",
                    Some(&format!("对账单 {} 确认结算", recon.doc_number)),
                )
                .await?;

            // 同步更新退货单实体表状态
            let rows = PurchaseReturnRepo::update_status(
                &mut *db,
                ret.id,
                PurchaseReturnStatus::Settled,
                &ret.updated_at,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if rows == 0 {
                return Err(DomainError::ConcurrentConflict);
            }

            // 发布退货结算事件，通知 FMS 生成贷项通知单
            new_domain_event_bus(self.pool.clone())
                .publish(
                    ctx, db,
                    EventPublishRequest {
                        event_type: DomainEventType::PurchaseReturnSettled,
                        aggregate_type: "PurchaseReturn".to_string(),
                        aggregate_id: ret.id,
                        payload: json!({
                            "reconciliation_id": id,
                            "reconciliation_doc_number": recon.doc_number,
                        }),
                        idempotency_key: None,
                    },
                )
                .await?;
        }

        // 6. 状态转换 Draft -> Confirmed
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Confirmed", None)
            .await?;

        // 7. 更新实体表状态
        let rows = PurchaseReconciliationRepo::update_status(
            &mut *db,
            id,
            PurchaseReconStatus::Confirmed,
            &recon.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 8. 发布领域事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::PurchaseReconciliationConfirmed,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({
                        "confirmed_amount": confirmed_amount.to_string(),
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 9. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn preview(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
        period: String,
    ) -> Result<Vec<PurchaseReconPreviewItem>> {
        // period 格式非法时宽松降级为空（严格校验留给 create 提交时）
        let (period_start, period_end) = match parse_period(&period) {
            Ok(p) => p,
            Err(_) => return Ok(vec![]),
        };

        let order_items = PurchaseOrderItemRepo::list_unreconciled_received_by_supplier(
            &mut *db,
            supplier_id,
            period_start,
            period_end,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(order_items
            .iter()
            .map(|item| {
                let amount = item.received_qty * item.unit_price;
                PurchaseReconPreviewItem {
                    order_id: item.order_id,
                    order_item_id: item.id,
                    product_id: item.product_id,
                    received_qty: item.received_qty,
                    returned_qty: item.returned_qty,
                    returned_amount: item.returned_qty * item.unit_price,
                    unit_price: item.unit_price,
                    amount,
                }
            })
            .collect())
    }
}

/// 解析 `YYYY-MM` 期间字符串为 (月初, 月末) `NaiveDate`。
fn parse_period(period: &str) -> Result<(NaiveDate, NaiveDate)> {
    let (year, month) = period
        .split_once('-')
        .and_then(|(y, m)| {
            let y: i32 = y.parse().ok()?;
            let m: u32 = m.parse().ok()?;
            Some((y, m))
        })
        .ok_or_else(|| {
            DomainError::validation(format!("期间格式错误，应为 YYYY-MM，实际: {}", period))
        })?;

    if !(1..=12).contains(&month) {
        return Err(DomainError::validation(format!("月份无效: {}", month)));
    }

    let period_start = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| DomainError::validation(format!("无效的期间: {}", period)))?;
    let period_end = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap()
    .pred_opt()
    .unwrap();

    Ok((period_start, period_end))
}
