//! 采购需求池 — Service trait

use async_trait::async_trait;

use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;

/// 采购需求池服务 — 查询外购需求 + 创建采购订单草稿
#[async_trait]
pub trait PurchaseDemandService: Send + Sync {
    /// 查询待处理的外购需求（订单行维度）
    async fn list_pending_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>>;

    /// 按物料聚合查询外购需求（物料维度 — 采购员操作入口）
    async fn list_material_aggregated(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>>;

    /// 按 id 集合批量查询需求详情（批量转单 drawer 汇总用）
    async fn get_demands_by_ids(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        ids: &[i64],
    ) -> Result<Vec<DemandSummary>>;

    /// 从选中的需求批量创建采购订单草稿
    ///
    /// **事务要求：** 调用方必须在事务中调用此方法（`tx.begin()` → 传 `&mut tx`）。
    /// 内部执行乐观锁 UPDATE → 创建 PO → 更新 target_doc → 发布事件，
    /// 任一步骤失败需整体回滚以避免需求成为孤儿状态。
    async fn create_order_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateOrderFromDemandsReq,
    ) -> Result<CreateDownstreamResult>;
}
