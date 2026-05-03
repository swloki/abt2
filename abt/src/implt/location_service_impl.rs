//! 库位服务实现
//!
//! 实现库位管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::{
    CreateLocationRequest, Location, LocationInventoryStats, LocationStatus, LocationWithWarehouse,
    UpdateLocationRequest, WarehouseInventoryStats,
};
use crate::repositories::{
    Executor, LocationRepo, PaginatedResult, PaginationParams, WarehouseRepo,
};
use crate::service::LocationService;

/// 库位服务实现
pub struct LocationServiceImpl {
    pool: Arc<PgPool>,
}

impl LocationServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LocationService for LocationServiceImpl {
    async fn create(&self, req: CreateLocationRequest, executor: Executor<'_>) -> Result<i64> {
        // 检查仓库是否存在
        if WarehouseRepo::find_by_id(&self.pool, req.warehouse_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("仓库不存在: {}", req.warehouse_id));
        }

        // 检查库位编码是否已存在
        if LocationRepo::code_exists_in_warehouse(&self.pool, req.warehouse_id, &req.location_code)
            .await?
        {
            return Err(anyhow::anyhow!("库位编码已存在: {}", req.location_code));
        }

        let location_id = LocationRepo::insert(
            executor,
            req.warehouse_id,
            &req.location_code,
            req.location_name.as_deref(),
            req.capacity,
        )
        .await?;

        Ok(location_id)
    }

    async fn update(
        &self,
        location_id: i64,
        req: UpdateLocationRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 检查库位是否存在
        let location = LocationRepo::find_by_id(&self.pool, location_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("库位不存在: {}", location_id))?;

        // 如果编码变更，检查新编码是否已存在
        if location.location_code != req.location_code
            && LocationRepo::code_exists_in_warehouse(
                &self.pool,
                location.warehouse_id,
                &req.location_code,
            )
            .await?
        {
            return Err(anyhow::anyhow!("库位编码已存在: {}", req.location_code));
        }

        LocationRepo::update(
            executor,
            location_id,
            &req.location_code,
            req.location_name.as_deref(),
            req.capacity,
        )
        .await
    }

    async fn update_status(
        &self,
        location_id: i64,
        is_active: bool,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 检查库位是否存在
        if LocationRepo::find_by_id(&self.pool, location_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("库位不存在: {}", location_id));
        }

        let status = if is_active { LocationStatus::Active } else { LocationStatus::Inactive };
        LocationRepo::update_status(executor, location_id, &status.to_string()).await
    }

    async fn delete(
        &self,
        location_id: i64,
        hard_delete: bool,
        executor: Executor<'_>,
    ) -> Result<bool> {
        // 检查库位是否存在
        if LocationRepo::find_by_id(&self.pool, location_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("库位不存在: {}", location_id));
        }

        // 检查库位下是否有库存
        if LocationRepo::has_inventory(&self.pool, location_id).await? {
            return Err(anyhow::anyhow!("库位下存在库存，无法删除"));
        }

        if hard_delete {
            LocationRepo::hard_delete(executor, location_id).await?;
        } else {
            LocationRepo::soft_delete(executor, location_id).await?;
        }

        Ok(true)
    }

    async fn get_by_id(&self, location_id: i64) -> Result<Option<Location>> {
        LocationRepo::find_by_id(&self.pool, location_id).await
    }

    async fn get_with_warehouse(&self, location_id: i64) -> Result<Option<LocationWithWarehouse>> {
        LocationRepo::get_with_warehouse(&self.pool, location_id).await
    }

    async fn list_by_warehouse(&self, warehouse_id: i64) -> Result<Vec<Location>> {
        LocationRepo::list_by_warehouse(&self.pool, warehouse_id).await
    }

    async fn list_by_warehouse_paginated(
        &self,
        warehouse_id: i64,
        keyword: Option<String>,
        is_active: Option<bool>,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<PaginatedResult<Location>> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20);

        let (items, total) = LocationRepo::list_by_warehouse_paginated(
            &self.pool,
            warehouse_id,
            keyword.as_deref(),
            is_active,
            page,
            page_size,
        )
        .await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total, &pagination))
    }

    async fn find_by_code(
        &self,
        warehouse_id: i64,
        location_code: &str,
    ) -> Result<Option<Location>> {
        LocationRepo::find_by_code(&self.pool, warehouse_id, location_code).await
    }

    async fn get_warehouse_inventory_stats(
        &self,
        warehouse_id: i64,
    ) -> Result<WarehouseInventoryStats> {
        LocationRepo::get_warehouse_inventory_stats(&self.pool, warehouse_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("仓库不存在: {}", warehouse_id))
    }

    async fn get_location_inventory_stats(
        &self,
        location_id: i64,
    ) -> Result<LocationInventoryStats> {
        LocationRepo::get_location_inventory_stats(&self.pool, location_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("库位不存在: {}", location_id))
    }

    async fn list_location_stats_by_warehouse(
        &self,
        warehouse_id: i64,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<PaginatedResult<LocationInventoryStats>> {
        let page = page.unwrap_or(1).max(1);
        let page_size = page_size.unwrap_or(20).clamp(1, 100);

        let (items, total) = LocationRepo::list_location_stats_by_warehouse(
            &self.pool,
            warehouse_id,
            page,
            page_size,
        )
        .await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total, &pagination))
    }
}
