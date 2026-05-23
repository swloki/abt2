use anyhow::Result;
use async_trait::async_trait;

use crate::models::{Quotation, QuotationQuery};
use crate::repositories::{Executor, PaginatedResult};

pub struct CreateQuotationParams<'a> {
    pub customer_name: &'a str,
    pub contact_person: Option<&'a str>,
    pub contact_phone: Option<&'a str>,
    pub remark: Option<&'a str>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
    pub operator_id: Option<i64>,
}

pub struct UpdateQuotationParams<'a> {
    pub customer_name: &'a str,
    pub contact_person: Option<&'a str>,
    pub contact_phone: Option<&'a str>,
    pub remark: Option<&'a str>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

#[async_trait]
pub trait QuotationService: Send + Sync {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateQuotationParams<'_>,
        items: Vec<CreateQuotationItemParams>,
    ) -> Result<i64>;

    async fn update(
        &self,
        executor: Executor<'_>,
        quotation_id: i64,
        params: &UpdateQuotationParams<'_>,
        items: Vec<CreateQuotationItemParams>,
    ) -> Result<()>;

    async fn delete(&self, executor: Executor<'_>, quotation_id: i64) -> Result<()>;

    async fn get_by_id(&self, quotation_id: i64) -> Result<Option<Quotation>>;

    async fn list(&self, query: &QuotationQuery) -> Result<PaginatedResult<Quotation>>;

    async fn update_status(
        &self,
        executor: Executor<'_>,
        quotation_id: i64,
        status: i16,
    ) -> Result<()>;
}

pub struct CreateQuotationItemParams {
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: rust_decimal::Decimal,
    pub quantity: rust_decimal::Decimal,
    pub discount: rust_decimal::Decimal,
    pub remark: Option<String>,
}
