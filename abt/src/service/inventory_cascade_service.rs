//! 级联查询库存服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::CascadeInventoryResult;

/// 级联查询库存服务
#[async_trait]
pub trait InventoryCascadeService: Send + Sync {
    /// 级联查询产品的 BOM 引用和子节点库存
    async fn cascade_inventory(
        &self,
        product_id: Option<i64>,
        product_code: Option<String>,
        max_results: i32,
    ) -> Result<CascadeInventoryResult>;
}
