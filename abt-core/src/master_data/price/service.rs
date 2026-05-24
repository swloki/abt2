use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait ProductPriceService: Send + Sync {
    async fn update_price(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType, new_price: Decimal, remark: String) -> Result<(), DomainError>;
    async fn list_price_history(&self, ctx: ServiceContext<'_>, query: PriceQuery, page: PageParams) -> Result<PaginatedResult<PriceLogEntry>, DomainError>;
    async fn get_current_price(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType) -> Result<Option<Decimal>, DomainError>;
    async fn get_price_at(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType, as_of: DateTime<Utc>) -> Result<Option<Decimal>, DomainError>;
}
