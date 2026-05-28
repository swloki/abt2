pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{ProductWithoutPriceRow, StockExportRow, StockFilter, StockLedger, UpsertStockReq};
pub use service::StockLedgerService;

use sqlx::PgPool;

pub fn new_stock_ledger_service(pool: PgPool) -> impl StockLedgerService {
    implt::StockLedgerServiceImpl::new(pool)
}
