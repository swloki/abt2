use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor, Result, ServiceContext};

/// BOM 内联工序服务 —— per-BOM 工序行编辑与查询（工艺 + 产出 + 工作中心）。
///
/// routing 降级为 copy-on-write 模板：`apply_routing_to_bom` 一键把 `routing_steps`
/// 全字段拷到 `bom_operations`，拷完即解耦。
///
/// 实现约定（review nit）：repo 层全用 `sqlx::query_as::<Postgres, T>(r#"..."#)` 运行时字符串
/// （不经 `query!` 宏），与 `bom_routing_output` 前身一致 —— 这样 migration 与 `cargo clippy`
/// 解耦，可先写代码 clippy 闭环、再跑 migration 098 建表（运行时才需表存在）。
#[async_trait]
pub trait BomOperationService: Send + Sync {
    /// 列出某 BOM 全部工序行（按 step_order）
    async fn list_operations(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<BomOperation>>;

    /// 单行查找（BOM 成本取数 / 工单 load 预检用）
    async fn find_operation(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<Option<BomOperation>>;

    /// 逐行 upsert（by product_code + step_order）。
    /// 产出品 output_product_id 校验在 handler 层（list_non_leaf_product_ids_by_codes）。
    async fn upsert_operation(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpsertBomOperationReq,
    ) -> Result<()>;

    /// 删一行（同时清对应 bom_step_prices，防 step_order 复用错配 —— R-5）
    async fn delete_operation(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<()>;

    /// 整批替换（delete all + 级联清 bom_step_prices + insert），BOM 保存 handler 用。
    /// ★ 事务边界约定（review minor）：必须在外层事务内调用（传入 &mut *tx），
    ///   本方法内部不 begin/commit。参考 load_routings_from_template 注释模式。
    async fn replace_operations(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        ops: Vec<UpsertBomOperationReq>,
    ) -> Result<()>;

    /// copy-on-write 拷贝守卫（抄 ERPNext `get_routing` bom.py:488-501）：
    ///   - force=false：仅当该 BOM bom_operations 无行时从 routing_steps 全字段拷贝；
    ///     已有行则 Err（避免覆盖手工编辑）
    ///   - force=true：delete all bom_operations（+ 级联清 bom_step_prices）后重拷
    /// 返回拷贝行数。
    async fn apply_routing_to_bom(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        routing_id: i64,
        force: bool,
    ) -> Result<usize>;

    /// 运维：批量同步工序名（按 process_code JOIN labor_process_dicts 批量 UPDATE process_name）。
    /// 字典标准化后由 IE 手工触发，不做自动同步（保 copy-on-write 隔离）。
    async fn resync_process_names(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<usize>;

    /// 统计某 BOM 的工序行数（apply 守卫 / UI 三态防呆用）
    async fn count_operations(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<i64>;
}
