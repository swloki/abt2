use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{
    CountCycleCountReq, CreateCycleCountReq, CycleCount, CycleCountFilter,
};

#[async_trait]
pub trait CycleCountService: Send + Sync {
    /// 创建盘点单（Draft 状态）
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateCycleCountReq,
    ) -> Result<i64, DomainError>;

    /// 查询盘点单详情
    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<CycleCount, DomainError>;

    /// 分页查询盘点单
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: CycleCountFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<CycleCount>, DomainError>;

    /// 开始盘点：Draft -> Counting
    async fn start_count(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    /// 录入盘点数量（Counting 状态下）
    async fn count(&self, ctx: ServiceContext<'_>, req: CountCycleCountReq) -> Result<(), DomainError>;

    /// 完成盘点：Counting -> Completed
    async fn complete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    /// 执行调整：Completed -> Adjusted
    async fn adjust(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    /// 取消盘点：Draft|Counting -> Cancelled
    async fn cancel(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
