use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{
    Bin, BinFilter, CreateBinReq, CreateWarehouseReq, CreateZoneReq, UpdateWarehouseReq,
    Warehouse, WarehouseFilter, Zone,
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
}
