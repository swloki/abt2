use async_trait::async_trait;

use super::model::{CreatePaymentRequestRequest, PaymentRequest};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

#[async_trait]
pub trait PaymentRequestService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreatePaymentRequestRequest,
    ) -> Result<i64, DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PaymentRequest, DomainError>;

    async fn approve(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn mark_paid_by_fms(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        payment_doc_no: String,
    ) -> Result<(), DomainError>;
}
