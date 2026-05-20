//! 采购订单服务接口
//!
//! 定义采购订单管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{PurchaseOrderItemInput, PurchaseOrderQuery, PurchaseOrderWithItems};
use crate::models::PurchaseOrderDetail;
use crate::repositories::{Executor, PaginatedResult};

/// 采购订单服务接口
#[async_trait]
pub trait PurchaseOrderService: Send + Sync {
    /// 创建采购订单（含行项目），返回 po_id
    async fn create(
        &self,
        supplier_id: i64,
        order_type: i16,
        remark: Option<String>,
        operator_id: Option<i64>,
        items: Vec<PurchaseOrderItemInput>,
        executor: Executor<'_>,
    ) -> Result<i64>;

    /// 更新采购订单（含行项目）
    async fn update(
        &self,
        po_id: i64,
        supplier_id: i64,
        remark: Option<String>,
        items: Vec<PurchaseOrderItemInput>,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 删除采购订单（软删除）
    async fn delete(&self, po_id: i64, executor: Executor<'_>) -> Result<()>;

    /// 根据 ID 获取采购订单详情（含行项目）
    async fn get_by_id(&self, po_id: i64) -> Result<Option<PurchaseOrderWithItems>>;

    /// 分页查询采购订单列表
    async fn list(&self, query: PurchaseOrderQuery) -> Result<PaginatedResult<PurchaseOrderDetail>>;

    /// 更新采购订单状态
    async fn update_status(
        &self,
        po_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()>;
}
