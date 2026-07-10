use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{
    CountCycleCountReq, CreateCycleCountReq, CycleCount, CycleCountFilter, CycleCountItem,
};
use super::repo::CycleCountRepo;
use super::service::CycleCountService;
use crate::shared::document_sequence::{service::DocumentSequenceService, new_document_sequence_service};
use crate::shared::enums::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus, EventPublishRequest};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::error::DomainError;
use crate::shared::types::{PgExecutor, Result};
use crate::wms::enums::{CycleCountStatus, TransactionType};
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::{new_inventory_transaction_service, service::InventoryTransactionService};
use crate::wms::settings::{new_wms_settings_service, service::WmsSettingsService};

pub struct CycleCountServiceImpl {
    pool: PgPool,
}

impl CycleCountServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn status_name(s: CycleCountStatus) -> String {
        match s {
            CycleCountStatus::Draft => "Draft".to_string(),
            CycleCountStatus::Counting => "Counting".to_string(),
            CycleCountStatus::Completed => "Completed".to_string(),
            CycleCountStatus::Adjusted => "Adjusted".to_string(),
            CycleCountStatus::Cancelled => "Cancelled".to_string(),
            CycleCountStatus::PendingReview => "PendingReview".to_string(),
        }
    }

    /// 对每条差异明细生成 Adjustment 库存事务，同步更新台账（修复"盘点不调账"的核心 bug）。
    async fn apply_adjustment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        count: &CycleCount,
        items: &[CycleCountItem],
    ) -> Result<()> {
        let txn_svc = new_inventory_transaction_service(self.pool.clone());
        for item in items {
            if item.variance_qty == Decimal::ZERO {
                continue;
            }
            // record() 回写台账需要 zone_id（cycle_count_item 仅存 bin_id）
            let zone_id = CycleCountRepo::bin_zone_id(&mut *db, item.bin_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| {
                    DomainError::business_rule(format!(
                        "盘点明细 {} 的库位 {} 未关联库区，无法调账",
                        item.id, item.bin_id
                    ))
                })?;

            txn_svc
                .record(
                    ctx,
                    db,
                    RecordTransactionReq {
                        doc_number: None,
                        delivery_no: None,
                        source_doc_number: Some(count.doc_number.clone()),
                        transaction_type: TransactionType::Adjustment,
                        product_id: item.product_id,
                        warehouse_id: count.warehouse_id,
                        zone_id: Some(zone_id),
                        bin_id: Some(item.bin_id),
                        batch_no: item.batch_no.clone(),
                        // 正差异 → 正向入库；负差异 → 负向出库
                        quantity: item.variance_qty,
                        unit_cost: None,
                        source_type: "cycle_count".to_string(),
                        source_id: count.id,
                        remark: item.variance_reason.clone(),
                    },
                )
                .await?;
        }
        Ok(())
    }

    /// 计算差异金额 = Σ |variance_qty| × unit_cost（unit_cost 取台账行成本，缺失则 0）
    async fn compute_variance_amount(
        db: &mut sqlx::postgres::PgConnection,
        warehouse_id: i64,
        items: &[CycleCountItem],
    ) -> Result<Decimal> {
        let mut total = Decimal::ZERO;
        for item in items {
            if item.variance_qty == Decimal::ZERO {
                continue;
            }
            let unit_cost = CycleCountRepo::ledger_unit_cost(
                db,
                item.product_id,
                warehouse_id,
                item.bin_id,
                item.batch_no.as_deref(),
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .unwrap_or(Decimal::ZERO);
            total += item.variance_qty.abs() * unit_cost;
        }
        Ok(total)
    }
}

#[async_trait]
impl CycleCountService for CycleCountServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateCycleCountReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::validation("盘点单明细不能为空"));
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::CycleCount)
            .await
            .unwrap_or_else(|_| format!("CC{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let count = CycleCountRepo::insert(
            &mut *db,
            &doc_number,
            &req,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(count.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<CycleCount> {
        CycleCountRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))
    }

    async fn get_items(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        count_id: i64,
    ) -> Result<Vec<CycleCountItem>> {
        CycleCountRepo::get_items(&mut *db, count_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_items_by_count_ids(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        count_ids: &[i64],
    ) -> Result<Vec<CycleCountItem>> {
        CycleCountRepo::list_by_count_ids(&mut *db, count_ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: CycleCountFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<CycleCount>> {
        CycleCountRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn start_count(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Counting".to_string(),
            });
        }

        CycleCountRepo::update_status(&mut *db, id, CycleCountStatus::Counting)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn count(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CountCycleCountReq,
    ) -> Result<()> {
        let cc = CycleCountRepo::get_by_id(&mut *db, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if cc.status != CycleCountStatus::Counting {
            return Err(DomainError::business_rule(format!(
                "盘点单状态为 {}，无法录入盘点数量",
                Self::status_name(cc.status)
            )));
        }

        // 一次性获取所有明细，用于计算差异
        let items = CycleCountRepo::get_items(&mut *db, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        for item in &req.items {
            let cc_item = items
                .iter()
                .find(|i| i.id == item.item_id)
                .ok_or_else(|| DomainError::not_found("盘点明细"))?;

            let variance_qty = item.counted_qty - cc_item.system_qty;

            CycleCountRepo::update_item_counted(
                &mut *db,
                item.item_id,
                item.counted_qty,
                variance_qty,
                item.variance_reason.as_deref(),
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(())
    }

    async fn complete(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::Counting {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Completed".to_string(),
            });
        }

        // 计算并持久化差异金额，供 adjust() 阈值判断与前端展示
        let items = CycleCountRepo::get_items(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let variance_amount =
            Self::compute_variance_amount(&mut *db, count.warehouse_id, &items).await?;
        CycleCountRepo::update_variance_amount(&mut *db, id, variance_amount)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        CycleCountRepo::update_status(&mut *db, id, CycleCountStatus::Completed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn adjust(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::Completed {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Adjusted".to_string(),
            });
        }

        let threshold = new_wms_settings_service(self.pool.clone())
            .get(ctx, db)
            .await?
            .cycle_count_variance_threshold;

        if count.variance_amount > threshold {
            // 超阈值 → 待审批，暂不调账
            CycleCountRepo::update_status(&mut *db, id, CycleCountStatus::PendingReview)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            new_domain_event_bus(self.pool.clone())
                .publish(
                    ctx,
                    db,
                    EventPublishRequest {
                        event_type: DomainEventType::CycleCountReviewRequested,
                        aggregate_type: "CycleCount".to_string(),
                        aggregate_id: id,
                        payload: serde_json::json!({
                            "doc_number": count.doc_number,
                            "warehouse_id": count.warehouse_id,
                            "variance_amount": variance_to_json(count.variance_amount),
                            "threshold": variance_to_json(threshold),
                        }),
                        idempotency_key: None,
                    },
                )
                .await?;
            return Ok(());
        }

        // 阈值内 → 直接调账
        let items = CycleCountRepo::get_items(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        self.apply_adjustment(ctx, db, &count, &items).await?;
        CycleCountRepo::update_status(&mut *db, id, CycleCountStatus::Adjusted)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        CycleCountRepo::mark_items_adjusted(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    async fn approve(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::PendingReview {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Adjusted".to_string(),
            });
        }

        CycleCountRepo::mark_reviewed(&mut *db, id, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let items = CycleCountRepo::get_items(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        self.apply_adjustment(ctx, db, &count, &items).await?;
        CycleCountRepo::update_status(&mut *db, id, CycleCountStatus::Adjusted)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        CycleCountRepo::mark_items_adjusted(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    async fn reject(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::PendingReview {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Completed".to_string(),
            });
        }

        // 驳回 → 打回 Completed 重盘（不调账）
        CycleCountRepo::update_status(&mut *db, id, CycleCountStatus::Completed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    async fn cancel(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if !matches!(
            count.status,
            CycleCountStatus::Draft | CycleCountStatus::Counting | CycleCountStatus::Completed
        ) {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Cancelled".to_string(),
            });
        }

        CycleCountRepo::update_status(&mut *db, id, CycleCountStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}

/// Decimal → serde_json::Value（避免 to_string 损失，直接用字符串保留精度）
fn variance_to_json(d: Decimal) -> serde_json::Value {
    serde_json::Value::String(d.to_string())
}
