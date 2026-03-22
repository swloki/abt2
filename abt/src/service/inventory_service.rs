//! 库存服务接口
//!
//! 定义库存管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::{
    Inventory as InventoryModel, InventoryDetail, InventoryLogDetail, InventoryLogQuery,
    InventoryQuery, OperationType, SetSafetyStockRequest, StockChangeRequest, StockTransferRequest,
};
use crate::repositories::{Executor, PaginatedResult};

/// 库存变动日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLog {
    pub log_id: i64,
    pub inventory_id: i64,
    pub product_id: i64,
    pub location_id: i64,
    pub change_qty: Decimal,
    pub before_qty: Decimal,
    pub after_qty: Decimal,
    pub operation_type: OperationType,
    pub ref_order_type: Option<String>,
    pub ref_order_id: Option<String>,
    pub operator: Option<String>,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 库存服务接口
#[async_trait]
pub trait InventoryService: Send + Sync {
    /// 入库
    async fn stock_in(
        &self,
        req: StockChangeRequest,
        executor: Executor<'_>,
    ) -> Result<InventoryLog>;

    /// 出库
    async fn stock_out(
        &self,
        req: StockChangeRequest,
        executor: Executor<'_>,
    ) -> Result<InventoryLog>;

    /// 盘点调整（增量调整）
    async fn adjust(&self, req: StockChangeRequest, executor: Executor<'_>)
    -> Result<InventoryLog>;

    /// 盘点设置（直接设置为目标数量）
    async fn set_quantity(&self, req: StockChangeRequest, executor: Executor<'_>)
    -> Result<InventoryLog>;

    /// 库存调拨
    async fn transfer(
        &self,
        req: StockTransferRequest,
        executor: Executor<'_>,
    ) -> Result<(InventoryLog, InventoryLog)>;

    /// 设置安全库存
    async fn set_safety_stock(
        &self,
        req: SetSafetyStockRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 获取产品在所有库位的库存
    async fn get_by_product(&self, product_id: i64) -> Result<Vec<InventoryDetail>>;

    /// 获取库位下所有产品库存
    async fn get_by_location(&self, location_id: i64) -> Result<Vec<InventoryDetail>>;

    /// 查询低库存产品（库存 < 安全库存）
    async fn list_low_stock(&self) -> Result<Vec<InventoryDetail>>;

    /// 分页查询库存
    async fn query(&self, query: InventoryQuery) -> Result<PaginatedResult<InventoryDetail>>;

    /// 获取产品在指定库位的库存
    async fn get_by_product_location(
        &self,
        product_id: i64,
        location_id: i64,
    ) -> Result<Option<InventoryModel>>;

    // ========================================================================
    // 库存变动记录查询
    // ========================================================================

    /// 查询产品库存变动记录
    async fn list_logs_by_product(&self, product_id: i64) -> Result<Vec<InventoryLogDetail>>;

    /// 查询库位库存变动记录
    async fn list_logs_by_location(&self, location_id: i64) -> Result<Vec<InventoryLogDetail>>;

    /// 查询仓库库存变动记录
    async fn list_logs_by_warehouse(&self, warehouse_id: i64) -> Result<Vec<InventoryLogDetail>>;

    /// 分页查询变动记录
    async fn query_logs(
        &self,
        query: InventoryLogQuery,
    ) -> Result<PaginatedResult<InventoryLogDetail>>;
}
