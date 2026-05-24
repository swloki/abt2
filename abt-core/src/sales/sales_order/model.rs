use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 销售订单状态：7 states per 01-sales.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum SalesOrderStatus {
    Draft = 1,
    Confirmed = 2,
    InProduction = 3,
    PartiallyShipped = 4,
    Shipped = 5,
    Completed = 6,
    Cancelled = 7,
}

impl SalesOrderStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::InProduction),
            4 => Some(Self::PartiallyShipped),
            5 => Some(Self::Shipped),
            6 => Some(Self::Completed),
            7 => Some(Self::Cancelled),
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
            Self::InProduction => "InProduction",
            Self::PartiallyShipped => "PartiallyShipped",
            Self::Shipped => "Shipped",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for SalesOrderStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for SalesOrderStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for SalesOrderStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown SalesOrderStatus: {v}").into())
    }
}

impl Serialize for SalesOrderStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for SalesOrderStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown SalesOrderStatus: {v}")))
    }
}

/// 销售订单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SalesOrder {
    pub id: i64,
    pub doc_number: String,
    pub customer_id: i64,
    pub contact_id: i64,
    pub sales_rep_id: i64,
    pub order_date: NaiveDate,
    pub status: SalesOrderStatus,
    pub total_amount: Decimal,
    pub total_cost: Decimal,
    pub payment_terms: String,
    pub delivery_terms: String,
    pub delivery_address: String,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 销售订单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SalesOrderItem {
    pub id: i64,
    pub order_id: i64,
    pub line_no: i32,
    pub product_id: i64,
    pub description: String,
    pub quantity: Decimal,
    pub unit: String,
    pub unit_price: Decimal,
    pub unit_cost: Decimal,
    pub discount_rate: Decimal,
    pub amount: Decimal,
    pub shipped_qty: Decimal,
    pub returned_qty: Decimal,
    pub delivery_date: Option<NaiveDate>,
}

/// 手动创建订单请求
#[derive(Debug, Clone)]
pub struct CreateSalesOrderReq {
    pub customer_id: i64,
    pub contact_id: i64,
    pub items: Vec<CreateSalesOrderItemReq>,
    pub payment_terms: Option<String>,
    pub delivery_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: Option<String>,
}

/// 创建订单明细请求
#[derive(Debug, Clone)]
pub struct CreateSalesOrderItemReq {
    pub product_id: i64,
    pub description: Option<String>,
    pub quantity: Decimal,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub unit_cost: Option<Decimal>,
    pub discount_rate: Option<Decimal>,
    pub delivery_date: Option<NaiveDate>,
}

/// 更新订单头请求（不含明细）
#[derive(Debug, Clone, Default)]
pub struct UpdateSalesOrderReq {
    pub customer_id: Option<i64>,
    pub contact_id: Option<i64>,
    pub payment_terms: Option<String>,
    pub delivery_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: Option<String>,
}

/// 订单查询过滤
#[derive(Debug, Clone, Default)]
pub struct SalesOrderQuery {
    pub customer_id: Option<i64>,
    pub status: Option<SalesOrderStatus>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub keyword: Option<String>,
}

/// 明细行批量插入输入
pub struct SalesOrderItemInput {
    pub line_no: i32,
    pub product_id: i64,
    pub description: String,
    pub quantity: Decimal,
    pub unit: String,
    pub unit_price: Decimal,
    pub unit_cost: Decimal,
    pub discount_rate: Decimal,
    pub amount: Decimal,
    pub delivery_date: Option<NaiveDate>,
}
