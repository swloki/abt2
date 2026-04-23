//! 工序字典数据模型
//!
//! 全局工序主数据，供工艺路线引用。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 工序字典
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LaborProcessDict {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// 请求结构
// ============================================================================

/// 创建工序字典请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLaborProcessDictReq {
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
}

/// 更新工序字典请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLaborProcessDictReq {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
}

// ============================================================================
// 查询结构
// ============================================================================

/// 工序字典查询参数
#[derive(Debug, Clone, Default)]
pub struct ListLaborProcessDictQuery {
    pub keyword: Option<String>,
    pub page: u32,
    pub page_size: u32,
}
