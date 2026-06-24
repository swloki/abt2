use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PaginatedResult, PgExecutor};
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait ProductionReceiptService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateReceiptReq,
    ) -> Result<i64>;
    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionReceipt>;
    async fn get_detail_lookups(
        &self,
        db: PgExecutor<'_>,
        receipt: &ProductionReceipt,
    ) -> Result<super::model::ReceiptDetailLookups>;

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReceiptListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ReceiptListItem>>;

    /// 按工单 ID 查所有入库单（工作台聚合用，薄封装 repo）
    async fn list_by_work_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<ReceiptListItem>>;
    async fn get_unit_cost(&self, db: PgExecutor<'_>, product_id: i64) -> Result<rust_decimal::Decimal>;
    async fn get_fqc_status(&self, ctx: &ServiceContext, db: PgExecutor<'_>, receipt_id: i64) -> Result<super::model::FqcGate>;
}
