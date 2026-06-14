use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::*;
use crate::mes::work_order::model::WorkOrder;
use crate::shared::types::error::DomainError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductionPlan {
    pub id: i64,
    pub doc_number: String,
    pub plan_date: NaiveDate,
    pub plan_type: PlanType,
    pub status: PlanStatus,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductionPlanItem {
    pub id: i64,
    pub plan_id: i64,
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub sales_order_id: Option<i64>,
    pub sales_order_item_id: Option<i64>,
    pub bom_snapshot_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub work_center_id: Option<i64>,
    pub priority: i32,
    pub status: PlanItemStatus,
}

#[derive(Debug, Clone)]
pub struct CreatePlanReq {
    pub plan_type: PlanType,
    pub plan_date: NaiveDate,
    pub remark: Option<String>,
    pub items: Vec<CreatePlanItemReq>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreatePlanItemReq {
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub sales_order_id: Option<i64>,
    pub sales_order_item_id: Option<i64>,
    pub bom_snapshot_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub work_center_id: Option<i64>,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct PlanFilter {
    pub status: Option<PlanStatus>,
    pub plan_type: Option<PlanType>,
    pub keyword: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}

#[derive(Debug)]
pub struct BatchReleaseResult {
    pub plan_id: i64,
    pub successful_work_orders: Vec<WorkOrder>,
    pub failed_items: Vec<BatchFailure>,
    pub validations: Vec<ReleaseValidation>,
    pub total: i32,
}

#[derive(Debug)]
pub struct BatchFailure {
    pub index: i32,
    pub error: DomainError,
}

#[derive(Debug, Clone)]
pub struct PlanExtraStats {
    pub item_count: i64,
    pub sales_orders: String,
}

/// 下达预校验结果
#[derive(Debug, Clone)]
pub struct ReleaseValidation {
    pub plan_item_id: i64,
    pub product_id: i64,
    pub has_routing: bool,
    pub has_published_bom: bool,
    pub routing_id: Option<i64>,
    pub warnings: Vec<String>,
    pub material_shortages: Vec<MaterialShortage>,
}

/// 物料短缺信息
#[derive(Debug, Clone)]
pub struct MaterialShortage {
    pub product_id: i64,
    pub required_qty: Decimal,
    pub available_qty: Decimal,
    pub shortage_qty: Decimal,
}

/// 工单规划项：使用者从计划明细拆分/调参后的工单生成请求
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkOrderPlanItem {
    pub plan_item_id: i64,
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub routing_id: Option<i64>,
    pub work_center_id: Option<i64>,
}
