use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// BOM 计件单价 —— per-BOM-per-step（价归 IE/成本域，与工序 `bom_operations` 分表）。
///
/// 中国制造业计件制：同一台机不同产品计件不同，故单价必须落 per-BOM-per-step 行
/// （三家 ERP 教本是工时制，价归 work_center，无对应）。
/// 工单下达时首次填后保存，后续同 BOM 工单自动加载。
///
/// `quantity`（R-1）：单件产品在该工序的计件倍数（legacy `bom_labor_processes.quantity` 语义）。
///   - BOM 成本报告：单件人工成本 = `unit_price × quantity`
///   - 报工 wage_amount：`completed_qty × unit_price`（公式不变，quantity 是成本维度非报工维度）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomStepPrice {
    pub id: i64,
    pub product_code: String,
    pub step_order: i32,
    /// 该 BOM 该工序计件单价（空 = 未定价，待工单现场填后回写）
    pub unit_price: Option<Decimal>,
    /// 单件计件倍数（0 = 不计件；影响成本报告）
    pub quantity: Decimal,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 单价变更历史（R-15）—— 支持月度审计报告 + diff 幅度溯源。
/// `upsert_price` 每次追加一行（old_price → new_price）。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomStepPriceHistory {
    pub id: i64,
    pub product_code: String,
    pub step_order: i32,
    pub old_price: Option<Decimal>,
    pub new_price: Option<Decimal>,
    pub quantity: Decimal,
    /// 'work_order_release' / 'bom_editor' / 'migration'
    pub source_type: String,
    /// 工单填价时记录 wo_id；BOM 编辑器/migration 为 NULL
    pub source_wo_id: Option<i64>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
}
