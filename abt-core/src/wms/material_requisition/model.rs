use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::RequisitionStatus;

/// 领料单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MaterialRequisition {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub requisition_date: NaiveDate,
    pub status: RequisitionStatus,
    pub warehouse_id: i64,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 领料单行项目
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MaterialReqItem {
    pub id: i64,
    pub requisition_id: i64,
    pub product_id: i64,
    pub requested_qty: Decimal,
    pub issued_qty: Decimal,
    pub variance_qty: Decimal,
    pub bin_id: Option<i64>,
}

/// 发料请求（整单）
#[derive(Debug, Clone)]
pub struct IssueMaterialReq {
    pub id: i64,
    pub items: Vec<IssueItemReq>,
}

/// 发料请求（行项目）
#[derive(Debug, Clone)]
pub struct IssueItemReq {
    pub item_id: i64,
    pub issued_qty: Decimal,
    pub bin_id: Option<i64>,
}

/// 领料单查询过滤
#[derive(Debug, Clone, Default)]
pub struct RequisitionFilter {
    pub doc_number: Option<String>,
    pub status: Option<RequisitionStatus>,
    pub work_order_id: Option<i64>,
    pub warehouse_id: Option<i64>,
}

/// 手动创建领料单请求（非工单驱动）
#[derive(Debug, Clone)]
pub struct CreateManualReq {
    pub warehouse_id: i64,
    pub requisition_date: NaiveDate,
    pub remark: Option<String>,
    pub items: Vec<CreateManualItemReq>,
}

/// 手动创建领料单行项目请求
#[derive(Debug, Clone)]
pub struct CreateManualItemReq {
    pub product_id: i64,
    pub requested_qty: Decimal,
}
