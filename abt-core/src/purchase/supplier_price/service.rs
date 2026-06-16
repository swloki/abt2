use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::{PriceListQuery, PriceUpsertRequest, PriceView, SupplierProductPrice};
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor};
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

    /// 价格目录列表（带供应商名/产品名 JOIN，支持筛选 + 关键词 + 分页）
    async fn list_prices(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: PriceListQuery, page: PageParams,
    ) -> Result<PaginatedResult<PriceView>>;

    /// 单条价格视图（编辑回填）
    async fn get_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<PriceView>;

    /// 创建价格（完整字段），返回新记录 id
    async fn create_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        req: PriceUpsertRequest,
    ) -> Result<i64>;

    /// 更新价格（完整字段）
    async fn update_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64, req: PriceUpsertRequest,
    ) -> Result<()>;

    async fn delete_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        price_id: i64,
    ) -> Result<()>;
}
