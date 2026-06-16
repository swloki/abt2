use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::ReserveRequest;
use super::repo::InventoryReservationRepo;
use super::service::InventoryReservationService;
use crate::shared::enums::DocumentType;
use crate::shared::types::PgExecutor;
use crate::shared::types::batch::{BatchFailure, BatchResult};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct InventoryReservationServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl InventoryReservationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InventoryReservationService for InventoryReservationServiceImpl {
    /// ContinueOnError 模式 — 逐条 INSERT，失败不影响其他行
    async fn reserve(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        requests: Vec<ReserveRequest>,
    ) -> Result<BatchResult> {
        let total = requests.len() as i32;
        if requests.is_empty() {
            return Ok(BatchResult::atomic_ok(0));
        }

        let mut success_count = 0i32;
        let mut failed_items = Vec::new();

        for (i, req) in requests.iter().enumerate() {
            // 1. 序列化同一 product 的并发预留（单 key，跨仓库），防止超卖
            if let Err(e) =
                InventoryReservationRepo::lock_for_reserve(&mut *db, req.product_id).await
            {
                failed_items.push(BatchFailure {
                    index: i as i32,
                    error: DomainError::Internal(e.into()),
                });
                continue;
            }

            // 2. 部分预留：actual_qty = min(需求量, ATP)。设计提案 §3.2：
            //    「库存部分满足 → 预占现有库存（Partial Allocation），剩余量触发补货」。
            //    仅当 ATP <= 0（完全无库存）时才判定 INSUFFICIENT_STOCK 整行失败，
            //    不创建负库存。available_atp 查询本身的 DB 错误是系统级，用 ? 传播中止整个 reserve。
            let atp = InventoryReservationRepo::available_atp(
                &mut *db, req.product_id, req.warehouse_id,
            ).await?;
            let actual_qty = req.reserved_qty.min(atp);
            if actual_qty <= Decimal::ZERO {
                failed_items.push(BatchFailure {
                    index: i as i32,
                    error: DomainError::business_rule(format!(
                        "INSUFFICIENT_STOCK: product {} need {} but ATP available {}",
                        req.product_id, req.reserved_qty, atp
                    )),
                });
                continue;
            }

            // 3. INSERT 预留（部分预留时 actual_qty < req.reserved_qty）
            let mut partial = req.clone();
            partial.reserved_qty = actual_qty;
            match InventoryReservationRepo::insert(&mut *db, &partial).await {
                Ok(_) => success_count += 1,
                Err(e) => {
                    failed_items.push(BatchFailure {
                        index: i as i32,
                        error: DomainError::Internal(e.into()),
                    });
                }
            }
        }

        Ok(BatchResult::continue_on_error(success_count, failed_items, total))
    }

    async fn fulfill(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let affected = InventoryReservationRepo::fulfill(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!(
                "Active reservation #{id}"
            )));
        }

        Ok(())
    }

    async fn cancel(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let affected = InventoryReservationRepo::cancel(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!(
                "Active reservation #{id}"
            )));
        }

        Ok(())
    }

    async fn total_reserved(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal> {
        InventoryReservationRepo::total_reserved(&mut *db, product_id, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn reserved_qty_by_source(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<std::collections::HashMap<i64, Decimal>> {
        InventoryReservationRepo::reserved_qty_by_source(&mut *db, source_type, source_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn cancel_by_source(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<u64> {
        InventoryReservationRepo::cancel_by_source(&mut *db, source_type, source_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn fulfill_by_source_line(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: DocumentType,
        source_line_id: i64,
    ) -> Result<()> {
        InventoryReservationRepo::fulfill_by_source_line(&mut *db, source_type, source_line_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }
    async fn consume(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
        product_id: i64,
        qty: Decimal,
    ) -> Result<()> {
        InventoryReservationRepo::consume(&mut *db, source_type, source_id, product_id, qty)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }
}
