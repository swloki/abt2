use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::wms::enums::ArrivalStatus;

/// 来料通知实体 — 映射 arrival_notices 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArrivalNotice {
    pub id: i64,
    pub doc_number: String,
    pub purchase_order_id: Option<i64>,
    pub supplier_id: i64,
    pub arrival_date: NaiveDate,
    pub status: ArrivalStatus,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub delivery_note: Option<String>,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 来料通知明细实体 — 映射 arrival_notice_items 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArrivalNoticeItem {
    pub id: i64,
    pub notice_id: i64,
    pub order_item_id: Option<i64>,
    pub product_id: i64,
    pub declared_qty: Decimal,
    pub received_qty: Decimal,
    pub accepted_qty: Decimal,
    pub batch_no: Option<String>,
}

/// 创建来料通知请求
#[derive(Debug, Clone)]
pub struct CreateArrivalNoticeReq {
    pub purchase_order_id: Option<i64>,
    pub supplier_id: i64,
    pub arrival_date: NaiveDate,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub delivery_note: Option<String>,
    pub remark: String,
    pub items: Vec<CreateArrivalNoticeItemReq>,
}

/// 创建来料通知明细请求
#[derive(Debug, Clone)]
pub struct CreateArrivalNoticeItemReq {
    pub order_item_id: Option<i64>,
    pub product_id: i64,
    pub declared_qty: Decimal,
    pub batch_no: Option<String>,
}

/// 收货请求
#[derive(Debug, Clone)]
pub struct ReceiveArrivalNoticeReq {
    pub id: i64,
    pub items: Vec<ReceiveItemReq>,
}

/// 收货明细请求
#[derive(Debug, Clone)]
pub struct ReceiveItemReq {
    pub item_id: i64,
    pub received_qty: Decimal,
    pub batch_no: Option<String>,
}

/// 检验请求
#[derive(Debug, Clone)]
pub struct InspectArrivalNoticeReq {
    pub id: i64,
    pub items: Vec<InspectItemReq>,
}

/// 检验明细请求
#[derive(Debug, Clone)]
pub struct InspectItemReq {
    pub item_id: i64,
    pub accepted_qty: Decimal,
}

/// 来料通知查询过滤
#[derive(Debug, Clone, Default)]
pub struct ArrivalNoticeFilter {
    pub doc_number: Option<String>,
    pub status: Option<ArrivalStatus>,
    pub supplier_id: Option<i64>,
    pub warehouse_id: Option<i64>,
}
