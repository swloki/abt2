use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{
    Bin, BinFilter, BinInventoryStats, BinWithWarehouse, CreateBinReq, CreateWarehouseReq,
    CreateZoneReq, ListBinsByWarehouseParams, SearchBinsParams, UpdateBinReq, UpdateWarehouseReq,
    UpdateZoneReq, Warehouse, WarehouseFilter, WarehouseInventoryStats, Zone,
};

#[async_trait]
pub trait WarehouseService: Send + Sync {
    /// 创建仓库，返回新仓库 ID
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateWarehouseReq,
    ) -> Result<i64>;

    /// 按 ID 查询仓库，不存在则返回 NotFound
    async fn get(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Warehouse>;

    /// 分页查询仓库
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: WarehouseFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Warehouse>>;

    /// 更新仓库（仅更新提供的字段）
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateWarehouseReq,
    ) -> Result<()>;

    /// 软删除仓库
    async fn delete(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 在指定仓库下创建库区，返回新库区 ID
    async fn create_zone(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
        req: CreateZoneReq,
    ) -> Result<i64>;

    /// 按 ID 查询库区，不存在则返回 NotFound
    async fn get_zone(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Zone>;

    /// 查询仓库下的所有库区
    async fn list_zones(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> Result<Vec<Zone>>;

    /// 更新库区（仅更新提供的字段）
    async fn update_zone(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateZoneReq,
    ) -> Result<()>;

    /// 软删除库区
    async fn delete_zone(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 在指定库区下创建库位，返回新库位 ID
    async fn create_bin(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        zone_id: i64,
        req: CreateBinReq,
    ) -> Result<i64>;

    /// 分页查询库区下的库位
    async fn list_bins(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        zone_id: i64,
        filter: Option<BinFilter>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Bin>>;

    /// 更新库位（仅更新提供的字段）
    async fn update_bin(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateBinReq,
    ) -> Result<()>;

    /// 软删除库位
    async fn delete_bin(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 跨 zone 查询仓库下所有 bin（分页，兼容旧 Location API）
    async fn list_bins_by_warehouse(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        params: ListBinsByWarehouseParams,
    ) -> Result<PaginatedResult<Bin>>;

    /// 查找或创建默认库区（用于兼容旧 Location API 的自动 zone 分配）
    async fn get_or_create_default_zone(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> Result<Zone>;

    /// 获取 bin 并关联仓库信息
    async fn get_bin_with_warehouse(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bin_id: i64,
    ) -> Result<BinWithWarehouse>;

    /// 跨仓库搜索 bin（带仓库名，分页）
    async fn search_bins_with_warehouse(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        params: SearchBinsParams,
    ) -> Result<PaginatedResult<BinWithWarehouse>>;

    /// 获取所有 bin 及仓库信息（无分页）
    async fn list_all_bins_with_warehouse(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
    ) -> Result<Vec<BinWithWarehouse>>;

    /// 仓库库存统计汇总
    async fn get_warehouse_inventory_stats(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> Result<WarehouseInventoryStats>;

    /// 库位库存统计
    async fn get_bin_inventory_stats(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bin_id: i64,
    ) -> Result<BinInventoryStats>;

    /// 分页获取仓库下所有库位的库存统计
    async fn list_bin_stats_by_warehouse(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<BinInventoryStats>>;
}
