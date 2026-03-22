//! 库位服务接口
//!
//! 定义库位管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{
    CreateLocationRequest, Location, LocationInventoryStats, LocationWithWarehouse,
    UpdateLocationRequest, WarehouseInventoryStats,
};
use crate::repositories::{Executor, PaginatedResult};

/// 库位服务接口
#[async_trait]
pub trait LocationService: Send + Sync {
    /// 创建库位
    async fn create(&self, req: CreateLocationRequest, executor: Executor<'_>) -> Result<i64>;

    /// 更新库位
    async fn update(
        &self,
        location_id: i64,
        req: UpdateLocationRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 删除库位（软删除或硬删除）
    async fn delete(
        &self,
        location_id: i64,
        hard_delete: bool,
        executor: Executor<'_>,
    ) -> Result<bool>;

    /// 获取库位详情
    async fn get_by_id(&self, location_id: i64) -> Result<Option<Location>>;

    /// 获取库位及仓库信息
    async fn get_with_warehouse(&self, location_id: i64) -> Result<Option<LocationWithWarehouse>>;

    /// 获取仓库下所有库位
    async fn list_by_warehouse(&self, warehouse_id: i64) -> Result<Vec<Location>>;

    /// 按编码查找库位
    async fn find_by_code(
        &self,
        warehouse_id: i64,
        location_code: &str,
    ) -> Result<Option<Location>>;

    // ========================================================================
    // 库存统计接口
    // ========================================================================

    /// 获取仓库库存统计汇总
    async fn get_warehouse_inventory_stats(
        &self,
        warehouse_id: i64,
    ) -> Result<WarehouseInventoryStats>;

    /// 获取库位库存统计
    async fn get_location_inventory_stats(
        &self,
        location_id: i64,
    ) -> Result<LocationInventoryStats>;

    /// 分页获取仓库下所有库位的库存统计
    async fn list_location_stats_by_warehouse(
        &self,
        warehouse_id: i64,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<PaginatedResult<LocationInventoryStats>>;
}
