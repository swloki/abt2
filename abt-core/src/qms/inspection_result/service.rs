use async_trait::async_trait;

use crate::qms::enums::QualityGateStatus;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};
use super::model::*;

#[async_trait]
pub trait InspectionResultService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateInspectionResultReq,
    ) -> Result<i64, DomainError>;

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<InspectionResult, DomainError>;

    /// 记录检验结果 — 录入实际数据并完成 Pending→Completed，返回质量关卡状态
    async fn record_result(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: RecordInspectionResultReq,
    ) -> Result<QualityGateStatus, DomainError>;

    async fn list_by_source(
        &self,
        ctx: ServiceContext<'_>,
        filter: InspectionResultFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<InspectionResult>, DomainError>;
}
