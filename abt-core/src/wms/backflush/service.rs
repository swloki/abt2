use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::{BackflushFilter, BackflushItem, BackflushRecord};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait BackflushService: Send + Sync {
    /// 执行冲扣（设计签名：execute(ctx, work_order_id, completed_qty)）
    /// 内部通过 WorkOrderStub 获取 BOM 并自动计算差异
    async fn execute(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
        completed_qty: Decimal,
    ) -> Result<i64>;

    /// 查询单条冲扣记录
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<BackflushRecord>;

    /// 查询冲扣明细列表
    async fn get_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        record_id: i64,
    ) -> Result<Vec<BackflushItem>>;

    /// 分页查询冲扣记录
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: BackflushFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<BackflushRecord>>;

    /// 调整：Executed -> Adjusted
    async fn adjust(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
