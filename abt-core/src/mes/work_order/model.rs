use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::super::enums::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkOrder {
    pub id: i64,
    pub doc_number: String,
    pub plan_item_id: Option<i64>,
    pub product_id: i64,
    pub bom_snapshot_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub status: WorkOrderStatus,
    pub work_center_id: Option<i64>,
    pub sales_order_id: Option<i64>,
    pub version: i32,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkOrderReq {
    pub plan_item_id: Option<i64>,
    pub product_id: i64,
    pub bom_snapshot_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub work_center_id: Option<i64>,
    pub sales_order_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkOrderFilter {
    pub status: Option<WorkOrderStatus>,
    pub product_id: Option<i64>,
    pub keyword: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}
