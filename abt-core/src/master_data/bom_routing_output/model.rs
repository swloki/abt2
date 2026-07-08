use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// BOM 工艺产出覆盖 —— per-BOM-per-step 的「产出品 + 计件价 + 工作中心覆盖」。
///
/// 把 045/063 迁移焊进 `routing_steps` 的 `product_id`/`unit_price` 下沉到此层，
/// 让 `routing`（工艺模板）回归纯工艺结构、可跨产品复用。
/// 对应 Odoo `mrp.bom.byproduct.operation_id` / OFBiz `WorkEffortGoodStandard`。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomRoutingOutput {
    pub id: i64,
    /// 成品编码（与 `bom_routings.product_code` 对齐）
    pub product_code: String,
    pub routing_id: i64,
    /// 对齐 `routing_steps.step_order`（编辑约束保证稳定，见设计文档 §5.1）
    pub step_order: i32,
    /// 该工序产出的中间品；必须 ∈ 该 BOM 的非叶子节点（校验在 handler 层）
    pub output_product_id: Option<i64>,
    /// 该 BOM 该工序的计件单价（空 → 报"未定价"）
    pub unit_price: Option<Decimal>,
    /// 工作中心覆盖；空 → 用模板 `routing_steps.work_center_id`
    pub work_center_id: Option<i64>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// UPSERT 请求（by product_code + step_order）
#[derive(Debug, Clone)]
pub struct UpsertBomOutputReq {
    pub product_code: String,
    pub routing_id: i64,
    pub step_order: i32,
    pub output_product_id: Option<i64>,
    pub unit_price: Option<Decimal>,
    pub work_center_id: Option<i64>,
}

/// 工序步骤 + per-BOM 覆盖视图（前端编辑分区、详情页用）。
/// 扁平结构：模板工艺属性 + 该 BOM 的覆盖值，一次 JOIN 取齐。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StepWithOutput {
    pub step_order: i32,
    pub process_code: String,
    #[sqlx(default)]
    pub process_name: Option<String>,
    /// 模板默认工作中心
    pub template_work_center_id: Option<i64>,
    #[sqlx(default)]
    pub template_work_center_name: Option<String>,
    #[sqlx(default)]
    pub standard_time: Option<Decimal>,
    #[sqlx(default)]
    pub is_outsourced: bool,
    #[sqlx(default)]
    pub is_inspection_point: bool,
    // —— per-BOM 覆盖（无覆盖行时均为 None）——
    #[sqlx(default)]
    pub output_id: Option<i64>,
    #[sqlx(default)]
    pub output_product_id: Option<i64>,
    #[sqlx(default)]
    pub output_product_name: Option<String>,
    #[sqlx(default)]
    pub unit_price: Option<Decimal>,
    #[sqlx(default)]
    pub work_center_override_id: Option<i64>,
    #[sqlx(default)]
    pub work_center_override_name: Option<String>,
}

impl StepWithOutput {
    /// 该步是否已有 per-BOM 覆盖
    pub fn has_override(&self) -> bool {
        self.output_id.is_some()
    }
}
