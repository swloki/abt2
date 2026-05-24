use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{
    CreateTransferReq, InventoryTransfer, TransferFilter,
};

#[async_trait]
pub trait TransferService: Send + Sync {
    /// 创建调拨单（状态: Draft）
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateTransferReq,
    ) -> Result<i64, DomainError>;

    /// 查询调拨单（含明细）
    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<InventoryTransfer, DomainError>;

    /// 分页查询调拨单列表
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: TransferFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransfer>, DomainError>;

    /// 发货（Draft → InTransit）
    async fn dispatch(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError>;

    /// 完成（InTransit → Completed）
    async fn complete(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError>;

    /// 取消（Draft → Cancelled）
    async fn cancel(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError>;
}
