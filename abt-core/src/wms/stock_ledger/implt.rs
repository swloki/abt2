use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{StockFilter, StockLedger, UpsertStockReq};
use super::repo::StockLedgerRepo;
use super::service::StockLedgerService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

pub struct StockLedgerServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl StockLedgerServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StockLedgerService for StockLedgerServiceImpl {
    async fn upsert(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        req: UpsertStockReq,
    ) -> Result<()> {
        let result = StockLedgerRepo::upsert(&mut *db, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if result.quantity < Decimal::ZERO {
            return Err(DomainError::BusinessRule(
                "库存数量不能为负".to_string(),
            ));
        }

        Ok(())
    }

    async fn query(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>> {
        StockLedgerRepo::query(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn query_available(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal> {
        StockLedgerRepo::total_available(&mut *db, product_id, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
