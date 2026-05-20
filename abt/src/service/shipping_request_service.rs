//! 发货申请服务接口
//!
//! 定义发货申请管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use crate::models::{ShippingRequest, ShippingRequestQuery};
use crate::repositories::{Executor, PaginatedResult};

#[async_trait]
pub trait ShippingRequestService: Send + Sync {
    /// 创建发货申请
    async fn create(&self, operator_id: Option<i64>, request: ShippingRequest, executor: Executor<'_>) -> Result<i64>;
    /// 更新发货申请
    async fn update(&self, operator_id: Option<i64>, request: ShippingRequest, executor: Executor<'_>) -> Result<()>;
    /// 删除发货申请
    async fn delete(&self, request_id: i64, executor: Executor<'_>) -> Result<()>;
    /// 根据 ID 获取发货申请
    async fn get_by_id(&self, request_id: i64) -> Result<Option<ShippingRequest>>;
    /// 分页查询发货申请列表
    async fn list(&self, query: ShippingRequestQuery) -> Result<PaginatedResult<ShippingRequest>>;
    /// 更新发货申请状态
    async fn update_status(&self, request_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}
