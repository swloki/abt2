//! 工序字典服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::*;
use crate::repositories::Executor;

/// 工序字典服务接口
#[async_trait]
pub trait LaborProcessDictService: Send + Sync {
    /// 搜索工序字典（分页）
    async fn list(&self, query: ListLaborProcessDictQuery) -> Result<(Vec<LaborProcessDict>, i64)>;

    /// 创建工序字典
    async fn create(&self, req: CreateLaborProcessDictReq, executor: Executor<'_>) -> Result<i64>;

    /// 更新工序字典
    async fn update(&self, req: UpdateLaborProcessDictReq, executor: Executor<'_>) -> Result<()>;

    /// 删除工序字典
    async fn delete(&self, id: i64, executor: Executor<'_>) -> Result<u64>;
}
