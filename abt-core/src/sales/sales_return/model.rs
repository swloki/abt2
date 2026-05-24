use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 退货状态：7 states per 01-sales.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ReturnStatus {
    Draft = 1,
    Confirmed = 2,
    Received = 3,
    Inspecting = 4,
    Completed = 5,
    Cancelled = 6,
    Rejected = 7,
}

impl ReturnStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::Received),
            4 => Some(Self::Inspecting),
            5 => Some(Self::Completed),
            6 => Some(Self::Cancelled),
            7 => Some(Self::Rejected),
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
            Self::Received => "Received",
            Self::Inspecting => "Inspecting",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
            Self::Rejected => "Rejected",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for ReturnStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for ReturnStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ReturnStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ReturnStatus: {v}").into())
    }
}

impl Serialize for ReturnStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for ReturnStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ReturnStatus: {v}")))
    }
}

/// 退货处置方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ReturnDisposition {
    Restock = 1,
    Scrap = 2,
    Rework = 3,
}

impl ReturnDisposition {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Restock),
            2 => Some(Self::Scrap),
            3 => Some(Self::Rework),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for ReturnDisposition {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for ReturnDisposition {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ReturnDisposition {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ReturnDisposition: {v}").into())
    }
}

impl Serialize for ReturnDisposition {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for ReturnDisposition {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ReturnDisposition: {v}")))
    }
}

/// 销售退货实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SalesReturn {
    pub id: i64,
    pub doc_number: String,
    pub order_id: i64,
    pub shipping_request_id: i64,
    pub customer_id: i64,
    pub return_date: NaiveDate,
    pub status: ReturnStatus,
    pub return_reason: String,
    pub total_amount: Decimal,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 销售退货明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SalesReturnItem {
    pub id: i64,
    pub return_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub returned_qty: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub disposition: ReturnDisposition,
}

/// 创建退货请求
#[derive(Debug, Clone)]
pub struct CreateReturnReq {
    pub order_id: i64,
    pub shipping_request_id: i64,
    pub customer_id: i64,
    pub return_reason: String,
    pub items: Vec<CreateReturnItemReq>,
}

/// 创建退货明细请求
#[derive(Debug, Clone)]
pub struct CreateReturnItemReq {
    pub order_item_id: i64,
    pub returned_qty: Decimal,
    pub disposition: ReturnDisposition,
}

/// 退货查询过滤
#[derive(Debug, Clone, Default)]
pub struct ReturnQuery {
    pub order_id: Option<i64>,
    pub shipping_request_id: Option<i64>,
    pub customer_id: Option<i64>,
    pub status: Option<ReturnStatus>,
    pub keyword: Option<String>,
}

/// 明细行批量插入输入
pub struct ReturnItemInput {
    pub order_item_id: i64,
    pub product_id: i64,
    pub returned_qty: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub disposition: ReturnDisposition,
}
