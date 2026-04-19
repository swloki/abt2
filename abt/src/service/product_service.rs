//! 产品服务接口
//!
//! 定义产品管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::{Product, ProductQuery};
use crate::repositories::{BomReference, Executor};

/// 产品服务接口
#[async_trait]
pub trait ProductService: Send + Sync {
    /// 创建新产品
    async fn create(&self, product: Product, executor: Executor<'_>) -> Result<i64>;

    /// 更新产品
    async fn update(&self, product_id: i64, product: Product, executor: Executor<'_>)
    -> Result<()>;

    /// 删除产品
    async fn delete(&self, product_id: i64, executor: Executor<'_>) -> Result<()>;

    /// 根据 ID 查找产品
    async fn find(&self, product_id: i64) -> Result<Option<Product>>;

    /// 根据 ID 列表批量查找产品
    async fn find_by_ids(&self, product_ids: &[i64]) -> Result<Vec<Product>>;

    /// 查询产品列表
    async fn query(&self, query: ProductQuery) -> Result<(Vec<Product>, i64)>;

    /// 检查产品编码是否存在
    async fn exist_code(&self, pool: &PgPool, code: &str) -> Result<bool>;

    /// 生成唯一的产品编码
    /// 使用时间戳格式: x{timestamp}
    async fn generate_product_code(&self, pool: &PgPool) -> Result<String>;

    /// 检查产品是否被 BOM 使用
    /// 返回 (是否被使用, 使用的 BOM 列表, 总数)
    async fn check_product_usage(&self, product_id: i64, page: Option<u32>, page_size: Option<u32>) -> Result<(bool, Vec<BomReference>, i64)>;
}
