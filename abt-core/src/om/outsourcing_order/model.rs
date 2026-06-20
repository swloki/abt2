use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::om::enums::{OutsourcingStatus, OutsourcingType};

// ---------------------------------------------------------------------------
// Entity structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OutsourcingOrder {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub supplier_id: i64,
    pub product_id: i64,
    pub outsourcing_type: OutsourcingType,
    pub planned_qty: Decimal,
    pub completed_qty: Decimal,
    pub unit_price: Decimal,
    pub scheduled_date: Option<NaiveDate>,
    pub status: OutsourcingStatus,
    pub virtual_warehouse_id: i64,
    pub source_warehouse_id: Option<i64>,
    pub version: i32,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OutsourcingMaterial {
    pub id: i64,
    pub outsourcing_id: i64,
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub sent_qty: Decimal,
    pub returned_qty: Decimal,
    pub unit_cost: Decimal,
}

impl OutsourcingMaterial {
    pub fn in_transit_qty(&self) -> Decimal {
        self.sent_qty - self.returned_qty
    }
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct OutsourcingOrderQuery {
    pub status: Option<OutsourcingStatus>,
    pub supplier_id: Option<i64>,
    pub outsourcing_type: Option<OutsourcingType>,
    pub work_order_id: Option<i64>,
    pub date_range: Option<(NaiveDate, NaiveDate)>,
    pub keyword: Option<String>,
}

// ---------------------------------------------------------------------------
// Request structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OutsourcingMaterialItem {
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub unit_cost: Option<Decimal>,
}

pub struct CreateOutsourcingOrderReq {
    pub work_order_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub supplier_id: i64,
    pub product_id: i64,
    pub outsourcing_type: OutsourcingType,
    pub planned_qty: Decimal,
    pub unit_price: Decimal,
    pub scheduled_date: Option<NaiveDate>,
    pub virtual_warehouse_id: i64,
    pub source_warehouse_id: i64,
    pub remark: Option<String>,
    pub materials: Vec<OutsourcingMaterialItem>,
}

/// 委外单更新参数（不含 id 和 version）
pub struct UpdateOutsourcingParams<'a> {
    pub supplier_id: Option<i64>,
    pub planned_qty: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub scheduled_date: Option<NaiveDate>,
    pub remark: Option<&'a str>,
}

pub struct UpdateOutsourcingOrderReq {
    pub id: i64,
    pub expected_version: i32,
    pub supplier_id: Option<i64>,
    pub planned_qty: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub scheduled_date: Option<NaiveDate>,
    pub remark: Option<String>,
    pub materials: Option<Vec<OutsourcingMaterialItem>>,
}

pub struct SendOutsourcingReq {
    pub id: i64,
    pub expected_version: i32,
    pub remark: Option<String>,
}

pub struct ReceiveOutsourcingReq {
    pub id: i64,
    pub expected_version: i32,
    pub received_qty: Decimal,
    pub warehouse_id: Option<i64>,
    pub iqc_passed_qty: Option<Decimal>,
    pub remark: Option<String>,
}

pub struct ConvertToInternalReq {
    pub id: i64,
    pub expected_version: i32,
    pub remark: Option<String>,
}

pub struct CancelOutsourcingReq {
    pub id: i64,
    pub expected_version: i32,
    pub remark: Option<String>,
}
