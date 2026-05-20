//! 供应商价格服务接口
//!
//! 定义供应商价格管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::models::{SupplierPriceDetail, SupplierPriceQuery};
use crate::repositories::{Executor, PaginatedResult};

/// 供应商价格服务接口
#[async_trait]
pub trait SupplierPriceService: Send + Sync {
    /// 新增供应商价格（追加模式），返回 price_id
    async fn upsert(
        &self,
        supplier_id: i64,
        product_id: i64,
        unit_price: Decimal,
        valid_from: DateTime<Utc>,
        valid_until: DateTime<Utc>,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64>;

    /// 分页查询供应商价格列表
    async fn list(
        &self,
        query: SupplierPriceQuery,
    ) -> Result<PaginatedResult<SupplierPriceDetail>>;
}
