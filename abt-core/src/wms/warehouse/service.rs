use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{
    Bin, BinFilter, CreateBinReq, CreateWarehouseReq, CreateZoneReq, UpdateWarehouseReq,
    Warehouse, WarehouseFilter, Zone,
};

#[async_trait]
pub trait WarehouseService: Send + Sync {
    /// 创建仓库，返回新仓库 ID
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateWarehouseReq,
    ) -> Result<i64, DomainError>;

    /// 按 ID 查询仓库，不存在则返回 NotFound
    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Warehouse, DomainError>;

    /// 分页查询仓库
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: WarehouseFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Warehouse>, DomainError>;

    /// 更新仓库（仅更新提供的字段）
    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateWarehouseReq,
    ) -> Result<(), DomainError>;

    /// 软删除仓库
    async fn delete(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError>;

    /// 在指定仓库下创建库区，返回新库区 ID
    async fn create_zone(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
        req: CreateZoneReq,
    ) -> Result<i64, DomainError>;

    /// 查询仓库下的所有库区
    async fn list_zones(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
    ) -> Result<Vec<Zone>, DomainError>;

    /// 在指定库区下创建库位，返回新库位 ID
    async fn create_bin(
        &self,
        ctx: ServiceContext<'_>,
        zone_id: i64,
        req: CreateBinReq,
    ) -> Result<i64, DomainError>;

    /// 分页查询库区下的库位
    async fn list_bins(
        &self,
        ctx: ServiceContext<'_>,
        zone_id: i64,
        filter: Option<BinFilter>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Bin>, DomainError>;
}
