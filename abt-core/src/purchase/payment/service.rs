use async_trait::async_trait;

use super::model::{CreatePaymentRequestRequest, PaymentRequest};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

#[async_trait]
pub trait PaymentRequestService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreatePaymentRequestRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PaymentRequest>;

    async fn approve(&self, ctx: ServiceContext<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn mark_paid_by_fms(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        payment_doc_no: String,
        idempotency_key: Option<String>,
    ) -> Result<()>;
}
