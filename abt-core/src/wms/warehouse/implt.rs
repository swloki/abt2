use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{
    Bin, BinFilter, BinInventoryStats, BinWithWarehouse, CreateBinReq, CreateWarehouseReq,
    CreateZoneReq, UpdateBinReq, UpdateWarehouseReq, UpdateZoneReq, Warehouse, WarehouseFilter,
    WarehouseInventoryStats, Zone,
};
use super::repo::WarehouseRepo;
use super::service::WarehouseService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

pub struct WarehouseServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl WarehouseServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WarehouseService for WarehouseServiceImpl {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateWarehouseReq,
    ) -> Result<i64, DomainError> {
        let warehouse = WarehouseRepo::insert_warehouse(&mut *ctx.executor, &req, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(warehouse.id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Warehouse, DomainError> {
        WarehouseRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse #{id}")))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: WarehouseFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Warehouse>, DomainError> {
        WarehouseRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateWarehouseReq,
    ) -> Result<(), DomainError> {
        let affected = WarehouseRepo::update(&mut *ctx.executor, id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Warehouse #{id}")));
        }

        Ok(())
    }

    async fn delete(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let affected = WarehouseRepo::soft_delete(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Warehouse #{id}")));
        }

        Ok(())
    }

    async fn create_zone(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
        req: CreateZoneReq,
    ) -> Result<i64, DomainError> {
        // 验证仓库存在
        WarehouseRepo::get_by_id(&mut *ctx.executor, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse #{warehouse_id}")))?;

        let zone = WarehouseRepo::insert_zone(&mut *ctx.executor, warehouse_id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(zone.id)
    }

    async fn list_zones(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
    ) -> Result<Vec<Zone>, DomainError> {
        WarehouseRepo::list_zones(&mut *ctx.executor, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update_zone(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateZoneReq,
    ) -> Result<(), DomainError> {
        let affected = WarehouseRepo::update_zone(&mut *ctx.executor, id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Zone #{id}")));
        }

        Ok(())
    }

    async fn delete_zone(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let affected = WarehouseRepo::soft_delete_zone(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Zone #{id}")));
        }

        Ok(())
    }

    async fn create_bin(
        &self,
        ctx: ServiceContext<'_>,
        zone_id: i64,
        req: CreateBinReq,
    ) -> Result<i64, DomainError> {
        let bin = WarehouseRepo::insert_bin(&mut *ctx.executor, zone_id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(bin.id)
    }

    async fn list_bins(
        &self,
        ctx: ServiceContext<'_>,
        zone_id: i64,
        filter: Option<BinFilter>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Bin>, DomainError> {
        let f = filter.unwrap_or_default();
        WarehouseRepo::list_bins(&mut *ctx.executor, zone_id, &f, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update_bin(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateBinReq,
    ) -> Result<(), DomainError> {
        let affected = WarehouseRepo::update_bin(&mut *ctx.executor, id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Bin #{id}")));
        }

        Ok(())
    }

    async fn delete_bin(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let affected = WarehouseRepo::soft_delete_bin(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if affected == 0 {
            return Err(DomainError::not_found(format!("Bin #{id}")));
        }

        Ok(())
    }

    async fn list_bins_by_warehouse(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
        keyword: Option<String>,
        is_active: Option<bool>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<Bin>, DomainError> {
        WarehouseRepo::list_bins_by_warehouse(
            &mut *ctx.executor,
            warehouse_id,
            keyword.as_deref(),
            is_active,
            page,
            page_size,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_or_create_default_zone(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
    ) -> Result<Zone, DomainError> {
        // 验证仓库存在
        WarehouseRepo::get_by_id(&mut *ctx.executor, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse #{warehouse_id}")))?;

        // 查找已有默认库区
        if let Some(zone) = WarehouseRepo::find_default_zone(&mut *ctx.executor, warehouse_id)
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
        WarehouseRepo::insert_zone(&mut *ctx.executor, warehouse_id, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_bin_with_warehouse(
        &self,
        ctx: ServiceContext<'_>,
        bin_id: i64,
    ) -> Result<BinWithWarehouse, DomainError> {
        let (bin, warehouse_id, warehouse_name) =
            WarehouseRepo::get_bin_with_warehouse(&mut *ctx.executor, bin_id)
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
        ctx: ServiceContext<'_>,
        keyword: Option<String>,
        is_active: Option<bool>,
        warehouse_id: Option<i64>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<BinWithWarehouse>, DomainError> {
        WarehouseRepo::search_bins_with_warehouse(
            &mut *ctx.executor,
            keyword.as_deref(),
            is_active,
            warehouse_id,
            page,
            page_size,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_all_bins_with_warehouse(
        &self,
        ctx: ServiceContext<'_>,
    ) -> Result<Vec<BinWithWarehouse>, DomainError> {
        WarehouseRepo::list_all_bins_with_warehouse(&mut *ctx.executor)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_warehouse_inventory_stats(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
    ) -> Result<WarehouseInventoryStats, DomainError> {
        WarehouseRepo::get_warehouse_inventory_stats(&mut *ctx.executor, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Warehouse#{warehouse_id}")))
    }

    async fn get_bin_inventory_stats(
        &self,
        ctx: ServiceContext<'_>,
        bin_id: i64,
    ) -> Result<BinInventoryStats, DomainError> {
        WarehouseRepo::get_bin_inventory_stats(&mut *ctx.executor, bin_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("Bin#{bin_id}")))
    }

    async fn list_bin_stats_by_warehouse(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<BinInventoryStats>, DomainError> {
        WarehouseRepo::list_bin_stats_by_warehouse(&mut *ctx.executor, warehouse_id, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
