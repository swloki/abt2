use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::*;
use crate::shared::types::{PgExecutor, Result, ServiceContext};

/// BOM 计件单价服务 —— per-BOM-per-step 单价查询与回写。
///
/// 工单填价回写（release drawer）与 BOM 页直接定价共用 `upsert_price` 入口。
/// upsert 前校验 `bom_operations` 有对应行，拒「有价无工序」孤儿（review minor）。
/// 每次回写追加 `bom_step_price_history` 一行（R-15，月度审计 + diff 溯源）。
#[async_trait]
pub trait BomStepPriceService: Send + Sync {
    /// 工单 load / BOM 成本报告取数用 —— 全部单价行（含 quantity）
    async fn find_prices_by_product(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<BomStepPrice>>;

    /// 单行单价查询（工单 load 用）
    async fn find_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<Option<Decimal>>;

    /// ★ 工单填价回写 + BOM 定价共用入口（by product_code + step_order）。
    ///
    /// - upsert 前校验 bom_operations 有对应行，拒孤儿
    /// - 写 bom_step_prices（quantity 保留原值；新建行默认 1）
    /// - 写 bom_step_price_history（R-15）
    ///
    /// `source_type` / `source_wo_id` 用于审计溯源：
    ///   - 工单填价：`("work_order_release", Some(wo_id))`
    ///   - BOM 编辑器：`("bom_editor", None)`
    async fn upsert_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
        unit_price: Decimal,
        source_type: String,
        source_wo_id: Option<i64>,
    ) -> Result<()>;
}
