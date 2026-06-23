use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait AdjustmentService: Send + Sync {
    /// 创建应收/应付调整单（创建即过账：写 ar_ap_ledger，立即影响余额与账龄）
    async fn create_adjustment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateAdjustmentReq,
    ) -> Result<i64>;

    /// 查询单张调整单
    async fn get_adjustment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ArApAdjustment>;

    /// 调整单列表（分页，含往来方名称）
    async fn list_adjustments(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: AdjustmentFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<AdjustmentRow>>;
}
