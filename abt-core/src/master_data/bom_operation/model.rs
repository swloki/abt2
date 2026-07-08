use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// BOM 内联工序 —— per-BOM-per-step 自洽工序行（工艺 + 产出 + 工作中心）。
///
/// copy-on-write 与 routing 模板解耦：`apply_routing_to_bom` 一键拷贝后，
/// 编辑 routing 模板不会回流影响已拷贝的 `bom_operations` 行。
/// 对应 ERPNext `BOM Operation` / Odoo `mrp.bom.operation`。
///
/// 推翻并取代 clean break 的 `bom_routing_outputs` 覆盖层（工序 + 产出合进自洽行，
/// 不再分两张表）。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomOperation {
    pub id: i64,
    /// 成品编码（与 bom_routings/bom_labor_processes 对齐；per-root-product-code-per-step）
    pub product_code: String,
    /// BOM 内工序序号（BOM 自主，不再对齐 routing_steps）
    pub step_order: i32,
    pub process_code: String,
    /// 工序名（拷贝时 COALESCE(lpd.name, process_code) 物化落库；copy-on-write 后字典改名不自动同步）
    pub process_name: String,
    /// 权威工作中心 FK（bom_nodes.work_center VARCHAR 是遗留 free-text）
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,
    pub standard_cost: Option<Decimal>,
    pub allowed_loss_rate: Decimal,
    pub is_outsourced: bool,
    /// 免检不免工序
    pub is_inspection_point: bool,
    pub is_required: bool,
    /// 该工序产出的中间品（须 ∈ 该 product_code 下 BOM 非叶子节点，handler 层校验）
    pub output_product_id: Option<i64>,
    pub remark: Option<String>,
    /// 拷贝来源 routing（纯溯源；改 routing 不回流影响本行）；手工建为 NULL
    pub source_routing_id: Option<i64>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// UPSERT 请求（by product_code + step_order）。
///
/// 不含 `source_routing_id`（手工建的工序为 NULL；拷贝由 `apply_routing_to_bom` 内部设）。
/// `output_product_id` 校验「∈ 该 product_code 下 Draft+Published BOM 非叶子节点并集」
/// 放在 abt-web handler 层（持有 BomQueryService + 本服务双句柄）。
#[derive(Debug, Clone)]
pub struct UpsertBomOperationReq {
    pub product_code: String,
    pub step_order: i32,
    pub process_code: String,
    pub process_name: String,
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,
    pub standard_cost: Option<Decimal>,
    pub allowed_loss_rate: Decimal,
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
    pub is_required: bool,
    pub output_product_id: Option<i64>,
    pub remark: Option<String>,
}
