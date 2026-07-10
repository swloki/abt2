use async_trait::async_trait;

use super::model::{
    CreatePurchaseQuotationRequest, PurchaseQuotation, PurchaseQuotationItem, PurchaseQuotationQuery, QuotationComparison,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

#[async_trait]
pub trait PurchaseQuotationService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePurchaseQuotationRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PurchaseQuotation>;

    async fn activate(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: PurchaseQuotationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseQuotation>>;

    async fn compare(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Vec<QuotationComparison>>;
    async fn list_items(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<Vec<PurchaseQuotationItem>>;
    /// 批量取多个报价的明细（扁平 Vec，调用方按 quotation_id 分组）。
    async fn list_items_by_quotation_ids(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_ids: &[i64],
    ) -> Result<Vec<PurchaseQuotationItem>>;
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}