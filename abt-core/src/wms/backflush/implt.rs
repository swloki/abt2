use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{BackflushFilter, BackflushRecord, CreateBackflushItemReq, CreateBackflushReq};
use super::repo::BackflushRepo;
use super::service::BackflushService;
use crate::master_data::bom::service::BomQueryService;
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::service::CostEntryService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::{CostEntityType, CostType, DocumentType};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::BackflushStatus;
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::service::InventoryTransactionService;
use crate::mes::work_order::service::WorkOrderService;
use crate::shared::types::error::DomainError;

const DEFAULT_VARIANCE_THRESHOLD: Decimal = Decimal::from_parts(5, 0, 0, false, 2);

pub struct BackflushServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    inventory_transaction_svc: Arc<dyn InventoryTransactionService>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    cost_entry: Arc<dyn CostEntryService>,
    work_order: Arc<dyn WorkOrderService>,
    bom: Arc<dyn BomQueryService>,
}

impl BackflushServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        inventory_transaction_svc: Arc<dyn InventoryTransactionService>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        cost_entry: Arc<dyn CostEntryService>,
        work_order: Arc<dyn WorkOrderService>,
        bom: Arc<dyn BomQueryService>,
    ) -> Self {
        Self { pool, inventory_transaction_svc, doc_seq, cost_entry, work_order, bom }
    }
}

#[async_trait]
impl BackflushService for BackflushServiceImpl {
    async fn execute(
        &self,
        mut ctx: ServiceContext<'_>,
        work_order_id: i64,
        completed_qty: Decimal,
    ) -> Result<i64> {
        let backflush_date = chrono::Local::now().date_naive();
        let variance_threshold = DEFAULT_VARIANCE_THRESHOLD;

        let wo = self.work_order.find_by_id(ctx.reborrow(), work_order_id).await?;
        let product_id = wo.product_id;

        let doc_number = self.doc_seq.next_number(ctx.reborrow(), DocumentType::Backflush)
            .await
            .unwrap_or_else(|_| format!("BF{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        // 1. 插入冲扣记录（Draft 状态）
        let record = BackflushRepo::insert(
            &mut *ctx.executor,
            &CreateBackflushReq {
                doc_number,
                work_order_id,
                product_id,
                completed_qty,
                backflush_date,
                variance_threshold,
                operator_id: ctx.operator_id,
            },
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 2. 从 BOM 获取组件，计算差异并插入明细
        let bom_components = get_bom_components(&self.bom, ctx.reborrow(), &wo).await?;

        for component in &bom_components {
            let theoretical_qty = component.required_qty * completed_qty;
            let actual_qty = theoretical_qty;
            let variance_qty = actual_qty - theoretical_qty;
            let variance_rate = if theoretical_qty > Decimal::ZERO {
                variance_qty / theoretical_qty
            } else {
                Decimal::ZERO
            };
            let is_over_threshold = variance_rate.abs() > variance_threshold;

            BackflushRepo::insert_item(
                &mut *ctx.executor,
                &CreateBackflushItemReq {
                    record_id: record.id,
                    component_id: component.product_id,
                    theoretical_qty,
                    actual_qty,
                    variance_qty,
                    variance_rate,
                    is_over_threshold,
                },
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            // 超阈值 → CostEntry(损耗成本) [IndependentTx]
            if is_over_threshold {
                let period = chrono::Local::now().format("%Y-%m").to_string();
                let _ = self.cost_entry.create_entries(
                    ctx.reborrow(),
                    vec![EntryRequest {
                        entity_type: CostEntityType::WorkOrder,
                        entity_id: work_order_id,
                        cost_type: CostType::Scrap,
                        debit_amount: variance_qty.abs() * Decimal::ONE,
                        credit_amount: variance_qty.abs() * Decimal::ONE,
                        cost_center: None,
                        profit_center: None,
                        period,
                        source_type: DocumentType::Backflush,
                        source_id: record.id,
                    }],
                )
                .await;
            }

            // execute -> InventoryTransaction.record(Backflush)
            let _ = self.inventory_transaction_svc.record(
                ctx.reborrow(),
                RecordTransactionReq {
                    doc_number: None,
                    transaction_type: crate::wms::enums::TransactionType::Backflush,
                    product_id: component.product_id,
                    warehouse_id: 0,
                    zone_id: None,
                    bin_id: None,
                    batch_no: None,
                    quantity: -actual_qty,
                    unit_cost: None,
                    source_type: "backflush".to_string(),
                    source_id: record.id,
                    remark: None,
                },
            )
            .await;
        }

        // 3. 更新状态为 Executed
        BackflushRepo::update_status(
            &mut *ctx.executor,
            record.id,
            BackflushStatus::Executed,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(record.id)
    }

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<BackflushRecord> {
        BackflushRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BackflushRecord"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: BackflushFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<BackflushRecord>> {
        BackflushRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn adjust(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let record = BackflushRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("BackflushRecord"))?;

        if record.status != BackflushStatus::Executed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", record.status),
                to: "Adjusted".to_string(),
            });
        }

        BackflushRepo::update_status(&mut *ctx.executor, id, BackflushStatus::Adjusted)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}

/// 从工单的 BOM snapshot 获取组件列表
async fn get_bom_components(
    bom: &Arc<dyn BomQueryService>,
    ctx: ServiceContext<'_>,
    wo: &crate::mes::work_order::model::WorkOrder,
) -> Result<Vec<BomComponent>> {
    let bom_id = wo.bom_snapshot_id;
    if let Some(bom_id) = bom_id {
        let nodes = bom.get_leaf_nodes(ctx, bom_id).await?;
        Ok(nodes.into_iter().map(|n| BomComponent {
            product_id: n.product_id,
            required_qty: n.quantity,
        }).collect())
    } else {
        Ok(vec![])
    }
}

struct BomComponent {
    product_id: i64,
    required_qty: Decimal,
}
