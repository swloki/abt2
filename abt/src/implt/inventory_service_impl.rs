//! 库存服务实现
//!
//! 实现库存管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::{
    Inventory as InventoryModel, InventoryDetail, InventoryLogDetail, InventoryLogQuery,
    InventoryQuery, OperationType, SetSafetyStockRequest, StockChangeRequest, StockTransferRequest,
};
use crate::repositories::{
    Executor, InventoryRepo, LocationRepo, PaginatedResult, PaginationParams,
};
use crate::service::{InventoryLog, InventoryService};

/// 库存服务实现
pub struct InventoryServiceImpl {
    pool: Arc<PgPool>,
}

impl InventoryServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InventoryService for InventoryServiceImpl {
    async fn stock_in(
        &self,
        req: StockChangeRequest,
        executor: Executor<'_>,
    ) -> Result<InventoryLog> {
        // 验证库位是否存在
        if LocationRepo::find_by_id(&self.pool, req.location_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("库位不存在: {}", req.location_id));
        }

        // 验证数量为正数
        if req.quantity <= Decimal::ZERO {
            return Err(anyhow::anyhow!("入库数量必须为正数"));
        }

        // 获取或创建库存记录
        let (inventory_id, before_qty, _is_new) =
            InventoryRepo::get_or_create_for_update(executor, req.product_id, req.location_id)
                .await?;

        let after_qty = before_qty + req.quantity;

        // 更新库存
        InventoryRepo::update_quantity(executor, inventory_id, after_qty).await?;

        // 记录变动日志
        let log_id = InventoryRepo::insert_log(
            executor,
            inventory_id,
            req.product_id,
            req.location_id,
            req.quantity,
            before_qty,
            after_qty,
            &OperationType::In,
            req.ref_order_type.as_deref(),
            req.ref_order_id.as_deref(),
            req.operator.as_deref(),
            req.remark.as_deref(),
        )
        .await?;

        Ok(InventoryLog {
            log_id,
            inventory_id,
            product_id: req.product_id,
            location_id: req.location_id,
            change_qty: req.quantity,
            before_qty,
            after_qty,
            operation_type: OperationType::In,
            ref_order_type: req.ref_order_type,
            ref_order_id: req.ref_order_id,
            operator: req.operator,
            remark: req.remark,
            created_at: Utc::now(),
        })
    }

    async fn stock_out(
        &self,
        req: StockChangeRequest,
        executor: Executor<'_>,
    ) -> Result<InventoryLog> {
        // 验证库位是否存在
        if LocationRepo::find_by_id(&self.pool, req.location_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("库位不存在: {}", req.location_id));
        }

        // 验证数量为正数
        if req.quantity <= Decimal::ZERO {
            return Err(anyhow::anyhow!("出库数量必须为正数"));
        }

        // 获取库存记录
        let (inventory_id, before_qty, _is_new) =
            InventoryRepo::get_or_create_for_update(executor, req.product_id, req.location_id)
                .await?;

        let change_qty = -req.quantity; // 出库为负数
        let after_qty = before_qty + change_qty;

        // 校验库存是否足够
        if after_qty < Decimal::ZERO {
            return Err(anyhow::anyhow!(
                "库存不足: 当前 {}, 需要 {}",
                before_qty,
                req.quantity
            ));
        }

        // 更新库存
        InventoryRepo::update_quantity(executor, inventory_id, after_qty).await?;

        // 记录变动日志
        let log_id = InventoryRepo::insert_log(
            executor,
            inventory_id,
            req.product_id,
            req.location_id,
            change_qty,
            before_qty,
            after_qty,
            &OperationType::Out,
            req.ref_order_type.as_deref(),
            req.ref_order_id.as_deref(),
            req.operator.as_deref(),
            req.remark.as_deref(),
        )
        .await?;

        Ok(InventoryLog {
            log_id,
            inventory_id,
            product_id: req.product_id,
            location_id: req.location_id,
            change_qty,
            before_qty,
            after_qty,
            operation_type: OperationType::Out,
            ref_order_type: req.ref_order_type,
            ref_order_id: req.ref_order_id,
            operator: req.operator,
            remark: req.remark,
            created_at: Utc::now(),
        })
    }

    async fn adjust(
        &self,
        req: StockChangeRequest,
        executor: Executor<'_>,
    ) -> Result<InventoryLog> {
        // 验证库位是否存在
        if LocationRepo::find_by_id(&self.pool, req.location_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("库位不存在: {}", req.location_id));
        }

        // 获取库存记录
        let (inventory_id, before_qty, _is_new) =
            InventoryRepo::get_or_create_for_update(executor, req.product_id, req.location_id)
                .await?;

        let after_qty = before_qty + req.quantity;

        // 校验库存不能为负数
        if after_qty < Decimal::ZERO {
            return Err(anyhow::anyhow!(
                "调整后库存不能为负数: 当前 {}, 调整 {}",
                before_qty,
                req.quantity
            ));
        }

        // 更新库存
        InventoryRepo::update_quantity(executor, inventory_id, after_qty).await?;

        // 记录变动日志
        let log_id = InventoryRepo::insert_log(
            executor,
            inventory_id,
            req.product_id,
            req.location_id,
            req.quantity,
            before_qty,
            after_qty,
            &OperationType::Adjust,
            req.ref_order_type.as_deref(),
            req.ref_order_id.as_deref(),
            req.operator.as_deref(),
            req.remark.as_deref(),
        )
        .await?;

        Ok(InventoryLog {
            log_id,
            inventory_id,
            product_id: req.product_id,
            location_id: req.location_id,
            change_qty: req.quantity,
            before_qty,
            after_qty,
            operation_type: OperationType::Adjust,
            ref_order_type: req.ref_order_type,
            ref_order_id: req.ref_order_id,
            operator: req.operator,
            remark: req.remark,
            created_at: Utc::now(),
        })
    }

    async fn set_quantity(
        &self,
        req: StockChangeRequest,
        executor: Executor<'_>,
    ) -> Result<InventoryLog> {
        // 验证库位是否存在
        if LocationRepo::find_by_id(&self.pool, req.location_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("库位不存在: {}", req.location_id));
        }

        // 校验数量不能为负数
        if req.quantity < Decimal::ZERO {
            return Err(anyhow::anyhow!("库存数量不能为负数: {}", req.quantity));
        }

        // 获取库存记录
        let (inventory_id, before_qty, _is_new) =
            InventoryRepo::get_or_create_for_update(executor, req.product_id, req.location_id)
                .await?;

        // 直接设置为目标数量
        let after_qty = req.quantity;
        let change_qty = after_qty - before_qty;

        // 更新库存
        InventoryRepo::update_quantity(executor, inventory_id, after_qty).await?;

        // 记录变动日志
        let log_id = InventoryRepo::insert_log(
            executor,
            inventory_id,
            req.product_id,
            req.location_id,
            change_qty,
            before_qty,
            after_qty,
            &OperationType::Adjust,
            req.ref_order_type.as_deref(),
            req.ref_order_id.as_deref(),
            req.operator.as_deref(),
            req.remark.as_deref(),
        )
        .await?;

        Ok(InventoryLog {
            log_id,
            inventory_id,
            product_id: req.product_id,
            location_id: req.location_id,
            change_qty,
            before_qty,
            after_qty,
            operation_type: OperationType::Adjust,
            ref_order_type: req.ref_order_type,
            ref_order_id: req.ref_order_id,
            operator: req.operator,
            remark: req.remark,
            created_at: Utc::now(),
        })
    }

    async fn transfer(
        &self,
        req: StockTransferRequest,
        executor: Executor<'_>,
    ) -> Result<(InventoryLog, InventoryLog)> {
        // 验证源库位和目标库位
        let from_location = LocationRepo::find_by_id(&self.pool, req.from_location_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("源库位不存在: {}", req.from_location_id))?;

        let to_location = LocationRepo::find_by_id(&self.pool, req.to_location_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("目标库位不存在: {}", req.to_location_id))?;

        // 验证数量为正数
        if req.quantity <= Decimal::ZERO {
            return Err(anyhow::anyhow!("调拨数量必须为正数"));
        }

        // 验证源库位和目标库位不同
        if req.from_location_id == req.to_location_id {
            return Err(anyhow::anyhow!("源库位和目标库位不能相同"));
        }

        // 获取源库存并锁定
        let (from_inv_id, from_before_qty, _) =
            InventoryRepo::get_or_create_for_update(executor, req.product_id, req.from_location_id)
                .await?;

        let from_after_qty = from_before_qty - req.quantity;
        if from_after_qty < Decimal::ZERO {
            return Err(anyhow::anyhow!(
                "源库位库存不足: 当前 {}, 需要 {}",
                from_before_qty,
                req.quantity
            ));
        }

        // 更新源库存
        InventoryRepo::update_quantity(executor, from_inv_id, from_after_qty).await?;

        // 记录出库日志
        let out_log_id = InventoryRepo::insert_log(
            executor,
            from_inv_id,
            req.product_id,
            req.from_location_id,
            -req.quantity,
            from_before_qty,
            from_after_qty,
            &OperationType::Transfer,
            None,
            None,
            req.operator.as_deref(),
            Some(&format!("调拨至库位 {}", to_location.location_code)),
        )
        .await?;

        let out_log = InventoryLog {
            log_id: out_log_id,
            inventory_id: from_inv_id,
            product_id: req.product_id,
            location_id: req.from_location_id,
            change_qty: -req.quantity,
            before_qty: from_before_qty,
            after_qty: from_after_qty,
            operation_type: OperationType::Transfer,
            ref_order_type: None,
            ref_order_id: None,
            operator: req.operator.clone(),
            remark: Some(format!("调拨至库位 {}", to_location.location_code)),
            created_at: Utc::now(),
        };

        // 获取目标库存并锁定
        let (to_inv_id, to_before_qty, _) =
            InventoryRepo::get_or_create_for_update(executor, req.product_id, req.to_location_id)
                .await?;

        let to_after_qty = to_before_qty + req.quantity;

        // 更新目标库存
        InventoryRepo::update_quantity(executor, to_inv_id, to_after_qty).await?;

        // 记录入库日志
        let in_log_id = InventoryRepo::insert_log(
            executor,
            to_inv_id,
            req.product_id,
            req.to_location_id,
            req.quantity,
            to_before_qty,
            to_after_qty,
            &OperationType::Transfer,
            None,
            None,
            req.operator.as_deref(),
            Some(&format!("从库位 {} 调入", from_location.location_code)),
        )
        .await?;

        let in_log = InventoryLog {
            log_id: in_log_id,
            inventory_id: to_inv_id,
            product_id: req.product_id,
            location_id: req.to_location_id,
            change_qty: req.quantity,
            before_qty: to_before_qty,
            after_qty: to_after_qty,
            operation_type: OperationType::Transfer,
            ref_order_type: None,
            ref_order_id: None,
            operator: req.operator,
            remark: Some(format!("从库位 {} 调入", from_location.location_code)),
            created_at: Utc::now(),
        };

        Ok((out_log, in_log))
    }

    async fn set_safety_stock(
        &self,
        req: SetSafetyStockRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 验证库位是否存在
        if LocationRepo::find_by_id(&self.pool, req.location_id)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("库位不存在: {}", req.location_id));
        }

        InventoryRepo::set_safety_stock(executor, req.product_id, req.location_id, req.safety_stock)
            .await
    }

    async fn get_by_product(&self, product_id: i64) -> Result<Vec<InventoryDetail>> {
        InventoryRepo::get_details_by_product(&self.pool, product_id).await
    }

    async fn get_by_location(&self, location_id: i64) -> Result<Vec<InventoryDetail>> {
        InventoryRepo::get_details_by_location(&self.pool, location_id).await
    }

    async fn list_low_stock(&self) -> Result<Vec<InventoryDetail>> {
        InventoryRepo::list_low_stock(&self.pool).await
    }

    async fn query(&self, query: InventoryQuery) -> Result<PaginatedResult<InventoryDetail>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        // 使用 QueryBuilder 统一查询
        let (items, total) = InventoryRepo::query_details(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total, &pagination))
    }

    async fn get_by_product_location(
        &self,
        product_id: i64,
        location_id: i64,
    ) -> Result<Option<InventoryModel>> {
        InventoryRepo::get_by_product_location(&self.pool, product_id, location_id).await
    }

    async fn list_logs_by_product(&self, product_id: i64) -> Result<Vec<InventoryLogDetail>> {
        let query = InventoryLogQuery {
            product_id: Some(product_id),
            ..Default::default()
        };
        let (items, _) = InventoryRepo::query_logs_detail(&self.pool, &query).await?;
        Ok(items)
    }

    async fn list_logs_by_location(&self, location_id: i64) -> Result<Vec<InventoryLogDetail>> {
        let query = InventoryLogQuery {
            location_id: Some(location_id),
            ..Default::default()
        };
        let (items, _) = InventoryRepo::query_logs_detail(&self.pool, &query).await?;
        Ok(items)
    }

    async fn list_logs_by_warehouse(&self, warehouse_id: i64) -> Result<Vec<InventoryLogDetail>> {
        let query = InventoryLogQuery {
            warehouse_id: Some(warehouse_id),
            ..Default::default()
        };
        let (items, _) = InventoryRepo::query_logs_detail(&self.pool, &query).await?;
        Ok(items)
    }

    async fn query_logs(
        &self,
        query: InventoryLogQuery,
    ) -> Result<PaginatedResult<InventoryLogDetail>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let (items, total) = InventoryRepo::query_logs_detail(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total, &pagination))
    }
}
