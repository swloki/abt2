use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::SupplierProductPrice;
use super::repo::SupplierProductPriceRepo;
use super::service::SupplierPriceService;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

pub struct SupplierPriceServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl SupplierPriceServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SupplierPriceService for SupplierPriceServiceImpl {
    async fn match_best_price(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
        product_id: i64,
        quantity: Decimal,
    ) -> Result<Option<SupplierProductPrice>> {
        SupplierProductPriceRepo::match_best_price(&mut *db, supplier_id, product_id, quantity).await
    }

    async fn get_last_purchase_price(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Option<(Decimal, chrono::NaiveDate)>> {
        SupplierProductPriceRepo::get_last_purchase_price(&mut *db, product_id).await
    }

    async fn list_by_supplier(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
    ) -> Result<Vec<SupplierProductPrice>> {
        SupplierProductPriceRepo::list_by_supplier(&mut *db, supplier_id).await
    }

    async fn list_by_product(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Vec<SupplierProductPrice>> {
        SupplierProductPriceRepo::list_by_product(&mut *db, product_id).await
    }

    async fn create_price(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
        product_id: i64,
        price: Decimal,
        currency_code: String,
    ) -> Result<()> {
        SupplierProductPriceRepo::insert(&mut *db, supplier_id, product_id, price, &currency_code).await
    }

    async fn delete_price(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        price_id: i64,
    ) -> Result<()> {
        SupplierProductPriceRepo::delete_by_id(&mut *db, price_id).await
    }
}
