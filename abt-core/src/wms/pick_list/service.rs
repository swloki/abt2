use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait PickListService: Send + Sync {
    /// 从发货单生成拣货单（outbound `pick()` 调用）。
    /// 从 outbound 明细生成 pick_list + items，MVP：`picked_qty = requested_qty` 自动满拣。
    /// 返回拣货单 ID。
    async fn generate_from_outbound(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outbound_id: i64,
    ) -> Result<i64>;

    /// 拣货完成：Draft → Picked（记录 picked_at）
    async fn complete_pick(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 取消：Draft → Cancelled
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 按 ID 查询拣货单
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PickList>;

    /// 按发货单查询拣货单（1:1）
    async fn find_by_outbound(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outbound_id: i64,
    ) -> Result<Option<PickList>>;

    /// 查询拣货明细
    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        pick_list_id: i64,
    ) -> Result<Vec<PickListItem>>;

    /// 分页查询拣货单
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PickListQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PickList>>;
}
