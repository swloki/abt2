//! MES 需求池 — 请求/响应模型

use chrono::{NaiveDate, DateTime, Utc};
use rust_decimal::Decimal;

/// 需求查询参数（订单行维度）
#[derive(Debug, Clone, Default)]
pub struct DemandPoolQuery {
    pub status: Option<i16>,
    pub product_id: Option<i64>,
    pub order_id: Option<i64>,
    pub keyword: Option<String>,              // 模糊搜索物料名称/编码
    pub required_date_start: Option<NaiveDate>, // 日期范围起点
    pub required_date_end: Option<NaiveDate>,   // 日期范围终点
    pub sort: Option<String>,                   // 排序：urgency/qty/earliest/demand_count
}

/// 需求摘要（订单行维度）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DemandSummary {
    pub id: i64,
    pub order_id: i64,
    pub order_no: Option<String>,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub demand_status: i16,
    pub target_doc_id: Option<i64>,       // 关联下游单据 ID
    pub target_doc_type: Option<i16>,     // 关联下游单据类型 (7=PO,12=PP,10=WO,11=OM)
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Default)]
pub struct MaterialAggQuery {
    pub product_id: Option<i64>,
    pub keyword: Option<String>,              // 模糊搜索物料名称/编码
    pub required_date_start: Option<NaiveDate>, // 日期范围起点
    pub required_date_end: Option<NaiveDate>,   // 日期范围终点
    pub sort: Option<String>,                   // 排序：urgency/qty/earliest/demand_count
}

/// 物料聚合摘要（物料维度 — 计划员操作入口）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MaterialAggSummary {
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub total_demand_qty: Decimal,
    pub demand_count: i64,
    pub earliest_required_date: Option<NaiveDate>,
    pub latest_required_date: Option<NaiveDate>,
}

/// 单条需求的排程参数
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PlanDemandItemReq {
    pub demand_id: i64,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub priority: i32,
}

/// 被跳过的需求
#[derive(Debug, Clone)]
pub struct SkippedDemand {
    pub demand_id: i64,
    pub reason: String,
}

/// 从需求直接创建 Draft 工单请求（扁平化：废弃 PP 层，需求→工单直达）
#[derive(Debug, Clone)]
pub struct CreateWorkOrdersFromDemandsReq {
    pub demand_ids: Vec<i64>,
    pub remark: Option<String>,
    /// 每条需求的排程参数 — 可选，不填则使用默认排程
    pub items: Option<Vec<PlanDemandItemReq>>,
    /// 默认排程参数（当 items 未提供时使用）
    pub default_scheduled_start: Option<NaiveDate>,
    pub default_scheduled_end: Option<NaiveDate>,
}

/// 创建工单结果（扁平化：按 product 聚合，可能生成多个 Draft 工单）
#[derive(Debug, Clone)]
pub struct CreateWorkOrdersResult {
    pub wo_ids: Vec<i64>,
    pub processed_demand_count: usize,
    pub skipped_demands: Vec<SkippedDemand>,
    pub demand_status: String,
}

/// 乐观锁返回的已锁定需求数据
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LockedDemand {
    pub id: i64,
    pub product_id: i64,
    pub source_id: i64,
    pub source_line_id: i64,
    pub acquire_channel: i16,
    pub required_qty: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
}
