use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::ReceiveAndStockInReq;
use super::service::PurchaseStockInService;
use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::enums::CounterpartyType;
use crate::purchase::enums::PurchaseOrderStatus;
use crate::purchase::order::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
use crate::purchase::settings::model::PurchaseSettings;
use crate::purchase::settings::repo::PurchaseSettingsRepo;
use crate::shared::audit_log::{model::RecordAuditLogReq, new_audit_log_service, service::AuditLogService};
use crate::shared::cost_entry::{model::EntryRequest, new_cost_entry_service, service::CostEntryService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::{CostEntityType, CostType, DocumentType};
use crate::shared::idempotency::{new_idempotency_service, service::IdempotencyService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::{PgExecutor, Result};
use crate::wms::enums::TransactionType;
use crate::wms::inventory_transaction::{
    model::RecordTransactionReq, new_inventory_transaction_service, service::InventoryTransactionService,
};
use crate::wms::warehouse::{new_warehouse_service, service::WarehouseService};

pub struct PurchaseStockInServiceImpl {
    pool: PgPool,
}

impl PurchaseStockInServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseStockInService for PurchaseStockInServiceImpl {
    async fn receive_and_stock_in(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ReceiveAndStockInReq,
    ) -> Result<()> {
        if req.rows.is_empty() {
            return Err(DomainError::validation("请至少添加一行收货明细"));
        }

        // 1. 幂等防护（同 idempotency_key 重复提交只入库一次；try_claim 在调用方事务内，
        //    业务失败回滚则记录也回滚，允许重试）
        if let Some(key) = req.idempotency_key.as_deref()
            && !key.is_empty()
            && !new_idempotency_service(self.pool.clone()).try_claim(ctx, db, key).await?
        {
            return Ok(());
        }

        // 2. 读 PO + 明细 + 超收容差设置
        let po = PurchaseOrderRepo::get_by_id(db, req.po_id)
            .await?
            .ok_or_else(|| DomainError::not_found(format!("PurchaseOrder #{}", req.po_id)))?;
        let po_items = PurchaseOrderItemRepo::list_by_order_id(db, req.po_id).await?;
        let settings = PurchaseSettingsRepo::get(db)
            .await
            .unwrap_or_else(|_| PurchaseSettings::default());

        // 入库单号（RK-YYYY-MM-SEQ，本单库存流水共用）
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::StockReceipt)
            .await?;
        let inv_svc = new_inventory_transaction_service(self.pool.clone());
        let wh_svc = new_warehouse_service(self.pool.clone());

        // product_id → order_item_id 映射（order_item_id=0 时按 product 解析；
        // stock-in/create 多 PO 场景前端只传 product_id，work-center drawer 传精确 order_item_id）
        let prod_to_oi: std::collections::HashMap<i64, i64> =
            po_items.iter().map(|i| (i.product_id, i.id)).collect();

        // 3. 逐行：超收校验 → record 库存 → 增量累加 received_qty
        for row in &req.rows {
            let order_item_id = if row.order_item_id != 0 {
                row.order_item_id
            } else {
                *prod_to_oi.get(&row.product_id).ok_or_else(|| {
                    DomainError::validation(format!(
                        "收货行产品 {} 不属于采购订单 #{}",
                        row.product_id, req.po_id
                    ))
                })?
            };
            let item = po_items.iter().find(|i| i.id == order_item_id).ok_or_else(|| {
                DomainError::validation(format!(
                    "收货行 order_item_id={} 不属于采购订单 #{}",
                    order_item_id, req.po_id
                ))
            })?;

            // 超收校验（含容差，迁移自 ArrivalAcceptedHandler）
            let max_qty = item.quantity
                * (Decimal::ONE + settings.over_delivery_allowance_pct / Decimal::from(100));
            if item.received_qty + row.received_qty > max_qty {
                return Err(DomainError::validation(format!(
                    "订单行 {} 收货数量 {} 超过允许上限 {}（含 {}% 容差）",
                    item.line_no,
                    item.received_qty + row.received_qty,
                    max_qty,
                    settings.over_delivery_allowance_pct
                )));
            }

            // record 库存（zone/bin 缺省 → 仓库默认库位；source 关联 PO）
            let zone_id = wh_svc
                .get_or_create_default_zone(ctx, db, row.warehouse_id)
                .await
                .ok()
                .map(|z| z.id);
            let default_bin_id = if let Some(zid) = zone_id {
                wh_svc
                    .list_bins(ctx, db, zid, None, 1, 1)
                    .await
                    .ok()
                    .and_then(|r| r.items.first().map(|b| b.id))
            } else {
                None
            };
            inv_svc
                .record(
                    ctx,
                    db,
                    RecordTransactionReq {
                        doc_number: Some(doc_number.clone()),
                        delivery_no: req.delivery_note.clone(),
                        source_doc_number: Some(po.doc_number.clone()),
                        transaction_type: TransactionType::PurchaseReceipt,
                        product_id: row.product_id,
                        warehouse_id: row.warehouse_id,
                        zone_id,
                        bin_id: row.bin_id.or(default_bin_id),
                        batch_no: row.batch_no.clone(),
                        quantity: row.received_qty,
                        unit_cost: None,
                        source_type: "purchase_order".to_string(),
                        source_id: req.po_id,
                        remark: req.remark.clone(),
                    },
                )
                .await?;

            // 增量累加 received_qty（行锁，并发部分收货串行化）
            PurchaseOrderItemRepo::add_received_qty(db, order_item_id, row.received_qty).await?;
        }

        // 4. PO 状态流转（重读 items 拿最新 received_qty；>=quantity→Received，>0→PartiallyReceived）
        let po_items_after = PurchaseOrderItemRepo::list_by_order_id(db, req.po_id).await?;
        let all_received = po_items_after.iter().all(|i| i.received_qty >= i.quantity);
        let any_received = po_items_after.iter().any(|i| i.received_qty > Decimal::ZERO);
        let target_status = if all_received {
            PurchaseOrderStatus::Received
        } else if any_received {
            PurchaseOrderStatus::PartiallyReceived
        } else {
            return Ok(()); // 防御性：不应发生
        };

        if po.status != PurchaseOrderStatus::Received && po.status != target_status {
            let affected =
                PurchaseOrderRepo::update_status(db, req.po_id, target_status, &po.updated_at).await?;
            if affected == 0 {
                return Err(DomainError::ConcurrentConflict);
            }
            new_audit_log_service(self.pool.clone())
                .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "PurchaseOrder",
                        entity_id: req.po_id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({
                            "from": format!("{:?}", po.status),
                            "to": format!("{:?}", target_status),
                            "trigger": "PurchaseStockIn",
                        })),
                        context: None,
                    },
                )
                .await?;
        }

        // 5. 立应付（PO 维度 upsert：首次 insert，已存在 rewrite 金额；金额=Σ received×price）
        let ap_amount: Decimal = po_items_after
            .iter()
            .map(|i| i.received_qty * i.unit_price)
            .sum();
        if ap_amount > Decimal::ZERO {
            let period = chrono::Utc::now().format("%Y-%m").to_string();
            let today = chrono::Local::now().date_naive();
            let doc_no = po.doc_number.clone();
            let desc = format!("采购入库 {doc_no}");
            let inserted = ArApLedgerRepo::insert(
                db,
                &ArApLedgerInsert {
                    party_type: CounterpartyType::Supplier,
                    party_id: po.supplier_id,
                    source_type: DocumentType::PurchaseOrder,
                    source_id: req.po_id,
                    source_doc_no: &doc_no,
                    against_type: None,
                    against_id: None,
                    direction: LedgerDirection::Credit,
                    amount: ap_amount,
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
            if inserted.is_none() {
                // 已存在（多次部分收货）：重算金额（amount_applied=0 才允许；已核销报错）
                ArApLedgerRepo::rewrite_amount_by_source(
                    db,
                    DocumentType::PurchaseOrder,
                    req.po_id,
                    ap_amount,
                )
                .await?;
            }
        }

        // 6. 成本分录（材料成本，source=PO；amount=本次入库总量）
        let total_received: Decimal = req.rows.iter().map(|r| r.received_qty).sum();
        if total_received > Decimal::ZERO {
            let period = chrono::Local::now().format("%Y-%m").to_string();
            new_cost_entry_service(self.pool.clone())
                .create_entries(
                    ctx,
                    db,
                    vec![EntryRequest {
                        entity_type: CostEntityType::PurchaseOrder,
                        entity_id: req.po_id,
                        cost_type: CostType::Material,
                        debit_amount: total_received,
                        credit_amount: total_received,
                        cost_center: None,
                        profit_center: None,
                        period,
                        source_type: DocumentType::PurchaseOrder,
                        source_id: req.po_id,
                    }],
                )
                .await?;
        }

        Ok(())
    }
}
