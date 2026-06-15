use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::SupplierProductPrice;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

#[async_trait]
pub trait SupplierPriceService: Send + Sync {
    async fn match_best_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        supplier_id: i64, product_id: i64, quantity: Decimal,
    ) -> Result<Option<SupplierProductPrice>>;

    async fn get_last_purchase_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Option<(Decimal, chrono::NaiveDate)>>;

    async fn list_by_supplier(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        supplier_id: i64,
    ) -> Result<Vec<SupplierProductPrice>>;

    async fn list_by_product(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Vec<SupplierProductPrice>>;

    async fn create_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        supplier_id: i64, product_id: i64, price: Decimal, currency_code: String,
    ) -> Result<()>;

    async fn delete_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        price_id: i64,
    ) -> Result<()>;
}
