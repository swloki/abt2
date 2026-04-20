//! 劳务工序数据模型
//!
//! 三层模型：工序主表 → 工序组（含连接表）→ BOM 劳务成本

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ============================================================================
// 工序主表
// ============================================================================

/// 劳务工序
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LaborProcess {
    pub id: i64,
    pub name: String,
    pub unit_price: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// 工序组
// ============================================================================

/// 工序组
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LaborProcessGroup {
    pub id: i64,
    pub name: String,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 工序组成员（连接表）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LaborProcessGroupMember {
    pub group_id: i64,
    pub process_id: i64,
    pub sort_order: i32,
}

/// 包含成员列表的工序组（用于 API 响应）
#[derive(Debug, Clone)]
pub struct LaborProcessGroupWithMembers {
    pub group: LaborProcessGroup,
    pub members: Vec<LaborProcessGroupMember>,
}

// ============================================================================
// BOM 劳务成本
// ============================================================================

/// BOM 劳务成本明细
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BomLaborCost {
    pub id: i64,
    pub bom_id: i64,
    pub process_id: i64,
    pub quantity: Decimal,
    pub unit_price_snapshot: Option<Decimal>,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// BOM 劳务成本项（含工序信息，用于 API 响应）
#[derive(Debug, Clone)]
pub struct BomLaborCostItem {
    pub id: i64,
    pub process_id: i64,
    pub process_name: String,
    pub current_unit_price: Decimal,
    pub snapshot_unit_price: Option<Decimal>,
    pub quantity: Decimal,
    pub remark: Option<String>,
}

impl BomLaborCostItem {
    pub fn subtotal(&self) -> Decimal {
        self.current_unit_price * self.quantity
    }

    pub fn snapshot_subtotal(&self) -> Option<Decimal> {
        self.snapshot_unit_price.map(|p| p * self.quantity)
    }
}

// ============================================================================
// 查询结构
// ============================================================================

/// 工序查询参数
#[derive(Debug, Clone, Default)]
pub struct LaborProcessQuery {
    pub keyword: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

/// 工序组查询参数
#[derive(Debug, Clone, Default)]
pub struct LaborProcessGroupQuery {
    pub keyword: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

// ============================================================================
// 请求结构
// ============================================================================

/// 创建工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLaborProcessReq {
    pub name: String,
    pub unit_price: Decimal,
    pub remark: Option<String>,
}

/// 更新工序请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLaborProcessReq {
    pub id: i64,
    pub name: String,
    pub unit_price: Decimal,
    pub remark: Option<String>,
}

/// 创建工序组请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLaborProcessGroupReq {
    pub name: String,
    pub remark: Option<String>,
    pub members: Vec<LaborProcessGroupMemberInput>,
}

/// 更新工序组请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLaborProcessGroupReq {
    pub id: i64,
    pub name: String,
    pub remark: Option<String>,
    pub members: Vec<LaborProcessGroupMemberInput>,
}

/// 工序组成员输入
#[derive(Debug, Clone, Deserialize)]
pub struct LaborProcessGroupMemberInput {
    pub process_id: i64,
    pub sort_order: i32,
}

/// 设置 BOM 劳务成本请求
#[derive(Debug, Clone, Deserialize)]
pub struct SetBomLaborCostReq {
    pub bom_id: i64,
    pub process_group_id: i64,
    pub items: Vec<BomLaborCostItemInput>,
}

/// BOM 劳务成本项输入
#[derive(Debug, Clone, Deserialize)]
pub struct BomLaborCostItemInput {
    pub process_id: i64,
    pub quantity: Decimal,
    pub remark: Option<String>,
}

// ============================================================================
// 价格变更影响
// ============================================================================

/// 价格变更影响统计
#[derive(Debug, Clone)]
pub struct PriceChangeImpact {
    pub affected_bom_count: i64,
    pub affected_item_count: i64,
}

// ============================================================================
// Excel 导入导出
// ============================================================================

/// Excel 列定义常量（导入和导出共用，保证 round-trip 兼容）
pub const LABOR_PROCESS_EXCEL_COLUMNS: &[&str] = &["工序名称", "单价", "备注"];

/// 工序 Excel 导入结果
#[derive(Debug, Clone)]
pub struct LaborProcessImportResult {
    pub success_count: i32,
    pub failure_count: i32,
    pub skip_count: i32,
    pub results: Vec<LaborProcessImportRowResult>,
    pub affected_bom_count: i64,
}

/// 工序 Excel 导入单行结果
#[derive(Debug, Clone)]
pub struct LaborProcessImportRowResult {
    pub row_number: i32,
    pub process_name: String,
    pub operation: String, // "created", "updated", "unchanged", "error"
    pub error_message: String,
}
