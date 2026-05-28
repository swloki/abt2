use async_trait::async_trait;

use crate::qms::enums::QualityGateStatus;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};
use super::model::*;

#[async_trait]
pub trait InspectionResultService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateInspectionResultReq,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<InspectionResult>;

    /// 记录检验结果 — 录入实际数据并完成 Pending→Completed，返回质量关卡状态
    async fn record_result(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: RecordInspectionResultReq,
    ) -> Result<QualityGateStatus>;

    async fn list_by_source(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: InspectionResultFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<InspectionResult>>;
}
