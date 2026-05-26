use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 发货状态：5 states per 01-sales.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ShippingStatus {
    Draft = 1,
    Confirmed = 2,
    Picking = 3,
    Shipped = 4,
    Cancelled = 5,
}

impl ShippingStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::Picking),
            4 => Some(Self::Shipped),
            5 => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Confirmed => "Confirmed",
            Self::Picking => "Picking",
            Self::Shipped => "Shipped",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for ShippingStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for ShippingStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ShippingStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ShippingStatus: {v}").into())
    }
}

impl Serialize for ShippingStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for ShippingStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ShippingStatus: {v}")))
    }
}

/// 发货申请实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShippingRequest {
    pub id: i64,
    pub doc_number: String,
    pub order_id: i64,
    pub customer_id: i64,
    pub request_date: NaiveDate,
    pub expected_ship_date: Option<NaiveDate>,
    pub status: ShippingStatus,
    pub shipping_address: String,
    pub carrier: String,
    pub tracking_number: String,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 发货申请明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShippingRequestItem {
    pub id: i64,
    pub shipping_request_id: i64,
    pub line_no: i32,
    pub order_item_id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub requested_qty: Decimal,
    pub shipped_qty: Decimal,
    pub description: String,
}

/// 从订单创建发货请求
#[derive(Debug, Clone)]
pub struct CreateFromOrderReq {
    pub order_id: i64,
    pub expected_ship_date: Option<NaiveDate>,
    pub shipping_address: Option<String>,
    pub items: Vec<CreateShippingItemReq>,
}

/// 创建发货明细请求
#[derive(Debug, Clone)]
pub struct CreateShippingItemReq {
    pub order_item_id: i64,
    pub warehouse_id: i64,
    pub requested_qty: Decimal,
}

/// 更新发货申请请求
#[derive(Debug, Clone, Default)]
pub struct UpdateShippingReq {
    pub expected_ship_date: Option<NaiveDate>,
    pub shipping_address: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub remark: Option<String>,
}

/// 发货查询过滤
#[derive(Debug, Clone, Default)]
pub struct ShippingQuery {
    pub order_id: Option<i64>,
    pub status: Option<ShippingStatus>,
    pub keyword: Option<String>,
}

/// 明细行批量插入输入
pub struct ShippingItemInput {
    pub line_no: i32,
    pub order_item_id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub requested_qty: Decimal,
    pub description: String,
}
