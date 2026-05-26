use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{
    InventoryDetailView, InventoryQueryFilter, StockChangeReq, StockOperationResult,
    StockTransferReq, TransactionDetailView, TransactionLogFilter,
};

#[async_trait]
pub trait InventoryService: Send + Sync {
    // ── 写操作 ──

    async fn stock_in(
        &self,
        ctx: ServiceContext<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult, DomainError>;

    async fn stock_out(
        &self,
        ctx: ServiceContext<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult, DomainError>;

    async fn adjust(
        &self,
        ctx: ServiceContext<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult, DomainError>;

    async fn set_quantity(
        &self,
        ctx: ServiceContext<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult, DomainError>;

    async fn transfer(
        &self,
        ctx: ServiceContext<'_>,
        req: StockTransferReq,
    ) -> Result<(StockOperationResult, StockOperationResult), DomainError>;

    async fn set_safety_stock(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        bin_id: i64,
        safety_stock: Decimal,
    ) -> Result<(), DomainError>;

    // ── 读操作 ──

    async fn get_by_product(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<Vec<InventoryDetailView>, DomainError>;

    async fn get_by_bin(
        &self,
        ctx: ServiceContext<'_>,
        bin_id: i64,
    ) -> Result<Vec<InventoryDetailView>, DomainError>;

    async fn list_low_stock(
        &self,
        ctx: ServiceContext<'_>,
    ) -> Result<Vec<InventoryDetailView>, DomainError>;

    async fn query(
        &self,
        ctx: ServiceContext<'_>,
        filter: InventoryQueryFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryDetailView>, DomainError>;

    // ── 日志查询 ──

    async fn query_logs(
        &self,
        ctx: ServiceContext<'_>,
        filter: TransactionLogFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<TransactionDetailView>, DomainError>;

    async fn list_logs_by_product(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<Vec<TransactionDetailView>, DomainError>;

    async fn list_logs_by_bin(
        &self,
        ctx: ServiceContext<'_>,
        bin_id: i64,
    ) -> Result<Vec<TransactionDetailView>, DomainError>;

    async fn list_logs_by_warehouse(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
    ) -> Result<Vec<TransactionDetailView>, DomainError>;
}
