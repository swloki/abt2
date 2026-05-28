use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{
    Bin, BinFilter, BinInventoryStats, BinWithWarehouse, CreateBinReq, CreateWarehouseReq,
    CreateZoneReq, ListBinsByWarehouseParams, SearchBinsParams, UpdateBinReq, UpdateWarehouseReq,
    UpdateZoneReq, Warehouse, WarehouseFilter, WarehouseInventoryStats, Zone,
};
use super::repo::WarehouseRepo;
use super::service::WarehouseService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

pub struct WarehouseServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl WarehouseServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WarehouseService for WarehouseServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateWarehouseReq,
    ) -> Result<i64> {
        let warehouse = WarehouseRepo::insert_warehouse(&mut *db, &req, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(warehouse.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Warehouse> {
        WarehouseRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse #{id}")))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: WarehouseFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Warehouse>> {
        WarehouseRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateWarehouseReq,
    ) -> Result<()> {
        let affected = WarehouseRepo::update(&mut *db, id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Warehouse #{id}")));
        }

        Ok(())
    }

    async fn delete(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let affected = WarehouseRepo::soft_delete(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Warehouse #{id}")));
        }

        Ok(())
    }

    async fn create_zone(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
        req: CreateZoneReq,
    ) -> Result<i64> {
        // 验证仓库存在
        WarehouseRepo::get_by_id(&mut *db, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse #{warehouse_id}")))?;

        let zone = WarehouseRepo::insert_zone(&mut *db, warehouse_id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(zone.id)
    }

    async fn list_zones(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> Result<Vec<Zone>> {
        WarehouseRepo::list_zones(&mut *db, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update_zone(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateZoneReq,
    ) -> Result<()> {
        let affected = WarehouseRepo::update_zone(&mut *db, id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Zone #{id}")));
        }

        Ok(())
    }

    async fn delete_zone(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let affected = WarehouseRepo::soft_delete_zone(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Zone #{id}")));
        }

        Ok(())
    }

    async fn create_bin(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        zone_id: i64,
        req: CreateBinReq,
    ) -> Result<i64> {
        let bin = WarehouseRepo::insert_bin(&mut *db, zone_id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(bin.id)
    }

    async fn list_bins(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        zone_id: i64,
        filter: Option<BinFilter>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Bin>> {
        let f = filter.unwrap_or_default();
        WarehouseRepo::list_bins(&mut *db, zone_id, &f, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update_bin(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateBinReq,
    ) -> Result<()> {
        let affected = WarehouseRepo::update_bin(&mut *db, id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Bin #{id}")));
        }

        Ok(())
    }

    async fn delete_bin(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let affected = WarehouseRepo::soft_delete_bin(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Bin #{id}")));
        }

        Ok(())
    }

    async fn list_bins_by_warehouse(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        params: ListBinsByWarehouseParams,
    ) -> Result<PaginatedResult<Bin>> {
        WarehouseRepo::list_bins_by_warehouse(
            &mut *db,
            params.warehouse_id,
            params.keyword.as_deref(),
            params.is_active,
            params.page,
            params.page_size,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_or_create_default_zone(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> Result<Zone> {
        // 验证仓库存在
        WarehouseRepo::get_by_id(&mut *db, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse #{warehouse_id}")))?;

        // 查找已有默认库区
        if let Some(zone) = WarehouseRepo::find_default_zone(&mut *db, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
        {
            return Ok(zone);
        }

        // 创建默认库区
        use crate::wms::enums::ZoneType;
        let req = super::model::CreateZoneReq {
            code: "DEFAULT".to_string(),
            name: "默认库区".to_string(),
            zone_type: ZoneType::Storage,
            sort_order: Some(0),
            remark: Some("系统自动创建的默认库区".to_string()),
        };
        WarehouseRepo::insert_zone(&mut *db, warehouse_id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_bin_with_warehouse(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bin_id: i64,
    ) -> Result<BinWithWarehouse> {
        let (bin, warehouse_id, warehouse_name) =
            WarehouseRepo::get_bin_with_warehouse(&mut *db, bin_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found(format!("Bin #{bin_id}")))?;

        Ok(BinWithWarehouse {
            bin,
            warehouse_id,
            warehouse_name,
        })
    }

    async fn search_bins_with_warehouse(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        params: SearchBinsParams,
    ) -> Result<PaginatedResult<BinWithWarehouse>> {
        WarehouseRepo::search_bins_with_warehouse(
            &mut *db,
            params.keyword.as_deref(),
            params.is_active,
            params.warehouse_id,
            params.page,
            params.page_size,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_all_bins_with_warehouse(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
    ) -> Result<Vec<BinWithWarehouse>> {
        WarehouseRepo::list_all_bins_with_warehouse(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_warehouse_inventory_stats(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> Result<WarehouseInventoryStats> {
        WarehouseRepo::get_warehouse_inventory_stats(&mut *db, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse#{warehouse_id}")))
    }

    async fn get_bin_inventory_stats(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bin_id: i64,
    ) -> Result<BinInventoryStats> {
        WarehouseRepo::get_bin_inventory_stats(&mut *db, bin_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Bin#{bin_id}")))
    }

    async fn list_bin_stats_by_warehouse(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<BinInventoryStats>> {
        WarehouseRepo::list_bin_stats_by_warehouse(&mut *db, warehouse_id, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
