//! 仓库服务接口
//!
//! 定义仓库管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{
    CreateWarehouseRequest, UpdateWarehouseRequest, Warehouse, WarehouseWithLocations,
};
use crate::repositories::Executor;

/// 仓库服务接口
#[async_trait]
pub trait WarehouseService: Send + Sync {
    /// 创建仓库
    async fn create(&self, req: CreateWarehouseRequest, executor: Executor<'_>) -> Result<i64>;

    /// 更新仓库
    async fn update(
        &self,
        warehouse_id: i64,
        req: UpdateWarehouseRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 删除仓库（软删除或硬删除）
    async fn delete(
        &self,
        warehouse_id: i64,
        hard_delete: bool,
        executor: Executor<'_>,
    ) -> Result<bool>;

    /// 获取仓库详情
    async fn get_by_id(&self, warehouse_id: i64) -> Result<Option<Warehouse>>;

    /// 获取仓库列表（仅活跃）
    async fn list_active(&self) -> Result<Vec<Warehouse>>;

    /// 获取所有仓库
    async fn list_all(&self) -> Result<Vec<Warehouse>>;

    /// 获取仓库及其库位
    async fn get_with_locations(&self, warehouse_id: i64)
    -> Result<Option<WarehouseWithLocations>>;

    /// 根据编码查找仓库
    async fn find_by_code(&self, warehouse_code: &str) -> Result<Option<Warehouse>>;

    /// 检查编码是否已存在
    async fn code_exists(&self, warehouse_code: &str) -> Result<bool>;
}
