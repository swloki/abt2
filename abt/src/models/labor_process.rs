//! 劳务工序数据模型
//!
//! 扁平模型：每个产品独立管理自己的工序列表

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// BOM 工序（按产品管理）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BomLaborProcess {
    pub id: i64,
    pub product_code: String,
    pub process_code: Option<String>,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// 请求结构
// ============================================================================

/// 创建工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLaborProcessReq {
    pub product_code: String,
    pub process_code: Option<String>,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

/// 更新工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLaborProcessReq {
    pub id: i64,
    pub product_code: String,
    pub process_code: Option<String>,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

// ============================================================================
// 查询结构
// ============================================================================

/// 工序查询参数
#[derive(Debug, Clone, Default)]
pub struct ListLaborProcessQuery {
    pub product_code: String,
    pub keyword: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

// ============================================================================
// Excel 导入导出
// ============================================================================

/// Excel 列定义常量（导入和导出共用，保证 round-trip 兼容）
pub const LABOR_PROCESS_EXCEL_COLUMNS: &[&str] = &["产品编码", "工序编码", "工序名称", "单价", "数量", "排序", "备注"];

/// 工序 Excel 导入结果
#[derive(Debug, Clone)]
pub struct LaborProcessImportResult {
    pub success_count: i32,
    pub failure_count: i32,
    pub results: Vec<LaborProcessImportRowResult>,
    /// 每个产品的工艺路线信息
    pub routing_results: Vec<PerProductRoutingResult>,
}

/// 工序 Excel 导入单行结果
#[derive(Debug, Clone)]
pub struct LaborProcessImportRowResult {
    pub row_number: i32,
    pub process_name: String,
    pub operation: String, // "created", "updated", "error"
    pub error_message: String,
}

/// Excel 导入解析后的有效行
#[derive(Debug, Clone)]
pub struct ValidLaborProcessRow {
    pub row_number: i32,
    pub product_code: String,
    pub process_code: Option<String>,
    pub name: String,
    pub unit_price: rust_decimal::Decimal,
    pub quantity: rust_decimal::Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

/// 单个产品的工艺路线匹配/创建结果
#[derive(Debug, Clone)]
pub struct PerProductRoutingResult {
    pub product_code: String,
    pub auto_created_routing: bool,
    pub matched_existing_routing: bool,
    pub routing_name: Option<String>,
    pub routing_id: Option<i64>,
}
