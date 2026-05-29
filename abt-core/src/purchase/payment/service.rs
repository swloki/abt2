use async_trait::async_trait;

use super::model::{CreatePaymentRequestRequest, PaymentRequest, PaymentRequestQuery};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::PageParams;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait PaymentRequestService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePaymentRequestRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PaymentRequest>;

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: PaymentRequestQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PaymentRequest>>;

    async fn approve(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn mark_paid_by_fms(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        payment_doc_no: String,
        idempotency_key: Option<String>,
    ) -> Result<()>;
}
