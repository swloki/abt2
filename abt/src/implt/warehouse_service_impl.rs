//! 仓库服务实现
//!
//! 实现仓库管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::{
    CreateWarehouseRequest, UpdateWarehouseRequest, Warehouse, WarehouseWithLocations,
};
use crate::repositories::{Executor, WarehouseRepo};
use crate::service::WarehouseService;

/// 仓库服务实现
pub struct WarehouseServiceImpl {
    pool: Arc<PgPool>,
}

impl WarehouseServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WarehouseService for WarehouseServiceImpl {
    async fn create(&self, req: CreateWarehouseRequest, executor: Executor<'_>) -> Result<i64> {
        // 检查编码是否已存在
        if WarehouseRepo::code_exists(&self.pool, &req.warehouse_code).await? {
            return Err(anyhow::anyhow!("仓库编码已存在: {}", req.warehouse_code));
        }

        let warehouse_id =
            WarehouseRepo::insert(executor, &req.warehouse_name, &req.warehouse_code).await?;
        Ok(warehouse_id)
    }

    async fn update(
        &self,
        warehouse_id: i64,
        req: UpdateWarehouseRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 检查仓库是否存在
        let warehouse = WarehouseRepo::find_by_id(&self.pool, warehouse_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("仓库不存在: {}", warehouse_id))?;

        // 如果更新了编码，检查新编码是否已存在
        if let Some(ref new_code) = req.warehouse_code
            && new_code != &warehouse.warehouse_code
            && WarehouseRepo::code_exists(&self.pool, new_code).await?
        {
            return Err(anyhow::anyhow!("仓库编码已存在: {}", new_code));
        }

        WarehouseRepo::update(
            executor,
            warehouse_id,
            &req.warehouse_name,
            req.warehouse_code.as_deref(),
            &req.status.to_string(),
        )
        .await
    }

    async fn delete(
        &self,
        warehouse_id: i64,
        hard_delete: bool,
        executor: Executor<'_>,
    ) -> Result<bool> {
        // 检查仓库是否存在
        if WarehouseRepo::find_by_id(&self.pool, warehouse_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("仓库不存在: {}", warehouse_id));
        }

        if hard_delete {
            // 硬删除：检查是否有库位和库存
            if WarehouseRepo::has_locations(&self.pool, warehouse_id).await? {
                return Err(anyhow::anyhow!("仓库下存在库位，无法删除"));
            }
            if WarehouseRepo::has_inventory(&self.pool, warehouse_id).await? {
                return Err(anyhow::anyhow!("仓库下存在库存，无法删除"));
            }
            WarehouseRepo::hard_delete(executor, warehouse_id).await?;
        } else {
            // 软删除：也需要检查是否有库位
            if WarehouseRepo::has_locations(&self.pool, warehouse_id).await? {
                return Err(anyhow::anyhow!("仓库下存在库位，无法删除"));
            }
            WarehouseRepo::soft_delete(executor, warehouse_id).await?;
        }

        Ok(true)
    }

    async fn get_by_id(&self, warehouse_id: i64) -> Result<Option<Warehouse>> {
        WarehouseRepo::find_by_id(&self.pool, warehouse_id).await
    }

    async fn list_active(&self) -> Result<Vec<Warehouse>> {
        WarehouseRepo::list_active(&self.pool).await
    }

    async fn list_all(&self) -> Result<Vec<Warehouse>> {
        WarehouseRepo::list_all(&self.pool).await
    }

    async fn get_with_locations(
        &self,
        warehouse_id: i64,
    ) -> Result<Option<WarehouseWithLocations>> {
        let warehouse = WarehouseRepo::find_by_id(&self.pool, warehouse_id).await?;
        if warehouse.is_none() {
            return Ok(None);
        }
        let warehouse = warehouse.unwrap();

        // 获取仓库下的所有库位（LocationRepo 将在 location 模块实现后可用）
        let locations = vec![];

        Ok(Some(WarehouseWithLocations {
            warehouse_id: warehouse.warehouse_id,
            warehouse_name: warehouse.warehouse_name,
            warehouse_code: warehouse.warehouse_code,
            status: warehouse.status,
            locations,
        }))
    }

    async fn find_by_code(&self, warehouse_code: &str) -> Result<Option<Warehouse>> {
        WarehouseRepo::find_by_code(&self.pool, warehouse_code).await
    }

    async fn code_exists(&self, warehouse_code: &str) -> Result<bool> {
        WarehouseRepo::code_exists(&self.pool, warehouse_code).await
    }
}
