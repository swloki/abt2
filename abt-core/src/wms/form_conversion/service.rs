use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{ConversionFilter, CreateConversionReq, FormConversion};

#[async_trait]
pub trait FormConversionService: Send + Sync {
    /// 创建形态转换单（Draft 状态）
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateConversionReq,
    ) -> Result<i64>;

    /// 获取形态转换单（含行项目）
    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<FormConversion>;

    /// 分页查询形态转换单
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ConversionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<FormConversion>>;

    /// 完成形态转换单（Draft -> Completed）
    async fn complete(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;

    /// 取消形态转换单（Draft -> Cancelled）
    async fn cancel(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;
}
