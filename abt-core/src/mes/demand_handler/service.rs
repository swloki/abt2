//! MES 需求池 — Service trait

use async_trait::async_trait;

use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;

/// MES 需求池服务 — 查询自制需求 + 创建生产计划草稿
#[async_trait]
pub trait MesDemandService: Send + Sync {
    /// 查询待处理的自制需求（订单行维度）
    async fn list_pending_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>>;

    /// 按物料聚合查询自制需求（物料维度 — 计划员操作入口）
    async fn list_material_aggregated(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>>;

    /// 从选中的需求创建生产计划草稿
    ///
    /// **事务要求：** 调用方必须在事务中调用此方法（`tx.begin()` → 传 `&mut tx`）。
    /// 内部执行乐观锁 UPDATE → 创建计划 → 更新 target_doc → 发布事件，
    /// 任一步骤失败需整体回滚以避免需求成为孤儿状态。
    async fn create_plan_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePlanFromDemandsReq,
    ) -> Result<CreateDownstreamResult>;
}
