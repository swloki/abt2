use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{StockFilter, StockLedger, UpsertStockReq};

#[async_trait]
pub trait StockLedgerService: Send + Sync {
    async fn upsert(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: UpsertStockReq,
    ) -> Result<()>;

    async fn query(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>>;

    async fn query_available(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal>;
}
