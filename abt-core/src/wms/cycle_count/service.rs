use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{
    CountCycleCountReq, CreateCycleCountReq, CycleCount, CycleCountFilter,
    CycleCountItem,
};

#[async_trait]
pub trait CycleCountService: Send + Sync {
    /// 创建盘点单（Draft 状态）
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateCycleCountReq,
    ) -> Result<i64>;

    /// 查询盘点单详情
    async fn get(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<CycleCount>;

    /// 查询盘点单明细项
    async fn get_items(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        count_id: i64,
    ) -> Result<Vec<CycleCountItem>>;

    /// 分页查询盘点单
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: CycleCountFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<CycleCount>>;

    /// 开始盘点：Draft -> Counting
    async fn start_count(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 录入盘点数量（Counting 状态下）
    async fn count(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CountCycleCountReq) -> Result<()>;

    /// 完成盘点：Counting -> Completed（计算并记录差异金额 variance_amount）
    async fn complete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 执行调整：Completed ->（差异金额超阈值则 PendingReview 待审批；否则直接调账 -> Adjusted）
    async fn adjust(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 审批通过：PendingReview -> 调账 -> Adjusted
    async fn approve(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 审批驳回：PendingReview -> Completed（打回重盘，不调账）
    async fn reject(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 取消盘点：Draft|Counting -> Cancelled
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
