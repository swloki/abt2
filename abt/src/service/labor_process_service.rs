//! 劳务工序服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::*;
use crate::repositories::Executor;

/// 劳务工序服务接口
#[async_trait]
pub trait LaborProcessService: Send + Sync {
    /// 搜索工序（按产品）
    async fn list(&self, query: ListLaborProcessQuery) -> Result<(Vec<BomLaborProcess>, i64)>;

    /// 创建工序
    async fn create(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64>;

    /// 更新工序
    async fn update(&self, req: UpdateLaborProcessReq, executor: Executor<'_>) -> Result<()>;

    /// 删除工序
    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64>;
}
