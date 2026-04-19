//! 劳务工序服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::*;
use crate::repositories::Executor;

/// 劳务工序服务接口
#[async_trait]
pub trait LaborProcessService: Send + Sync {
    // ========================================================================
    // 工序 CRUD
    // ========================================================================

    /// 搜索工序
    async fn list_processes(&self, query: LaborProcessQuery) -> Result<(Vec<LaborProcess>, i64)>;

    /// 创建工序
    async fn create_process(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64>;

    /// 更新工序（返回价格变更影响统计）
    async fn update_process(
        &self,
        req: UpdateLaborProcessReq,
        executor: Executor<'_>,
    ) -> Result<Option<PriceChangeImpact>>;

    /// 删除工序（被组引用时由 FK RESTRICT 拒绝）
    async fn delete_process(&self, id: i64, executor: Executor<'_>) -> Result<u64>;

    // ========================================================================
    // 工序组 CRUD
    // ========================================================================

    /// 搜索工序组（含成员列表）
    async fn list_groups(&self, query: LaborProcessGroupQuery) -> Result<(Vec<LaborProcessGroupWithMembers>, i64)>;

    /// 创建工序组
    async fn create_group(&self, req: CreateLaborProcessGroupReq, executor: Executor<'_>) -> Result<i64>;

    /// 更新工序组
    async fn update_group(&self, req: UpdateLaborProcessGroupReq, executor: Executor<'_>) -> Result<()>;

    /// 删除工序组（被 BOM 引用时拒绝）
    async fn delete_group(&self, id: i64, executor: Executor<'_>) -> Result<u64>;

    // ========================================================================
    // BOM 劳务成本
    // ========================================================================

    /// 设置 BOM 劳务成本（清除旧的，批量插入新的，冻结当前价格到快照）
    async fn set_bom_labor_cost(&self, req: SetBomLaborCostReq, executor: Executor<'_>) -> Result<()>;

    /// 获取 BOM 劳务成本（含工序信息和价格快照对比）
    async fn get_bom_labor_cost(&self, bom_id: i64) -> Result<Option<(LaborProcessGroupWithMembers, Vec<BomLaborCostItem>)>>;
}
