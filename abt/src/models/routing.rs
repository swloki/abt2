//! 工艺路线数据模型
//!
//! 工艺路线是可复用的工序组合模板，可绑定到 BOM（产品）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 工艺路线
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Routing {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 路线工序明细
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RoutingStep {
    pub id: i64,
    pub routing_id: i64,
    pub process_code: String,
    pub step_order: i32,
    pub is_required: bool,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// BOM 路线映射
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BomRouting {
    pub id: i64,
    pub product_code: String,
    pub routing_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// 请求结构
// ============================================================================

/// 路线工序输入（创建/更新时使用）
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingStepInput {
    pub process_code: String,
    pub step_order: i32,
    pub is_required: bool,
    pub remark: Option<String>,
}

/// 创建工艺路线请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateRoutingReq {
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<RoutingStepInput>,
}

/// 更新工艺路线请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRoutingReq {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<RoutingStepInput>,
}

// ============================================================================
// 查询结构
// ============================================================================

/// 工艺路线查询参数
#[derive(Debug, Clone, Default)]
pub struct ListRoutingQuery {
    pub keyword: Option<String>,
    pub page: u32,
    pub page_size: u32,
}
