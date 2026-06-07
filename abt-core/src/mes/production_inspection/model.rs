use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::super::enums::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductionInspection {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub routing_id: Option<i64>,
    pub product_id: i64,
    pub inspection_type: InspectionType,
    pub sample_qty: Decimal,
    pub qualified_qty: Decimal,
    pub unqualified_qty: Decimal,
    pub result: InspectionResultType,
    pub inspector_id: i64,
    pub inspection_date: NaiveDate,
    pub disposition: Option<String>,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateInspectionReq {
    pub work_order_id: i64,
    pub product_id: i64,
    pub routing_id: Option<i64>,
    pub inspection_type: InspectionType,
    pub sample_qty: Decimal,
    pub inspection_date: NaiveDate,
    pub disposition: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InspectionDetailLookups {
    pub wo_doc_number: Option<String>,
    pub product_name: Option<String>,
    pub inspector_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InspectionListFilter {
    pub keyword: Option<String>,
    pub inspection_type: Option<InspectionType>,
}
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InspectionListItem {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub work_order_doc: Option<String>,
    pub routing_id: Option<i64>,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub inspection_type: InspectionType,
    pub sample_qty: Decimal,
    pub qualified_qty: Decimal,
    pub unqualified_qty: Decimal,
    pub result: InspectionResultType,
    pub inspector_id: i64,
    pub inspection_date: NaiveDate,
    pub disposition: Option<String>,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
