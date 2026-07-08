use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor, Result, ServiceContext};

/// BOM 工艺产出覆盖服务 —— per-BOM 的「产出品 + 计件价」编辑与查询。
///
/// 与 `RoutingService`（工艺模板）解耦：模板管可共享的工艺结构，
/// 本服务管每个 BOM 各自的产出/价格差异。校验「产出品 ∈ 该 BOM 非叶子节点」
/// 放在 abt-web handler 层（持有 BomQueryService + 本服务双句柄，避免 service 层循环依赖）。
#[async_trait]
pub trait BomRoutingOutputService: Send + Sync {
    /// 列出某 BOM 绑定 routing 的全部工序 + 覆盖状态（前端编辑分区 / 详情页用）。
    async fn list_steps_with_output(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<StepWithOutput>>;

    /// UPSERT 单道工序的产出覆盖（by product_code + step_order）。
    async fn upsert_output(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpsertBomOutputReq,
    ) -> Result<()>;

    /// 删除单道工序的产出覆盖（回退到模板默认）。
    async fn delete_output(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<()>;

    /// 按 product_code 取全部覆盖行（`load_routings_from_template` 等内部高效取数用）。
    async fn find_outputs_by_product(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<BomRoutingOutput>>;
}
