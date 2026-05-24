use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{StockFilter, StockLedger, UpsertStockReq};

#[async_trait]
pub trait StockLedgerService: Send + Sync {
    async fn upsert(
        &self,
        ctx: ServiceContext<'_>,
        req: UpsertStockReq,
    ) -> Result<(), DomainError>;

    async fn query(
        &self,
        ctx: ServiceContext<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>, DomainError>;

    async fn query_available(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal, DomainError>;
}
