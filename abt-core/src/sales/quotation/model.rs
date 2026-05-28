use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 报价单状态：1=Draft, 2=Sent, 3=Accepted, 4=Rejected, 5=Expired
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum QuotationStatus {
    Draft = 1,
    Sent = 2,
    Accepted = 3,
    Rejected = 4,
    Expired = 5,
}

impl QuotationStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Sent),
            3 => Some(Self::Accepted),
            4 => Some(Self::Rejected),
            5 => Some(Self::Expired),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Sent => "Sent",
            Self::Accepted => "Accepted",
            Self::Rejected => "Rejected",
            Self::Expired => "Expired",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for QuotationStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for QuotationStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for QuotationStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown QuotationStatus: {v}").into())
    }
}

impl Serialize for QuotationStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for QuotationStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown QuotationStatus: {v}")))
    }
}

/// 报价单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Quotation {
    pub id: i64,
    pub doc_number: String,
    pub customer_id: i64,
    pub contact_id: i64,
    pub sales_rep_id: i64,
    pub quotation_date: NaiveDate,
    pub valid_until: NaiveDate,
    pub status: QuotationStatus,
    pub total_amount: Decimal,
    pub total_cost: Decimal,
    pub estimated_margin: Decimal,
    pub payment_terms: String,
    pub delivery_terms: String,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 报价单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QuotationItem {
    pub id: i64,
    pub quotation_id: i64,
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

/// 创建报价单请求
#[derive(Debug, Clone)]
pub struct CreateQuotationReq {
    pub customer_id: i64,
    pub contact_id: i64,
    pub valid_until: NaiveDate,
    pub items: Vec<CreateQuotationItemReq>,
    pub payment_terms: Option<String>,
    pub delivery_terms: Option<String>,
    pub remark: Option<String>,
}

/// 创建报价单明细请求
#[derive(Debug, Clone)]
pub struct CreateQuotationItemReq {
    pub product_id: i64,
    pub description: Option<String>,
    pub quantity: Decimal,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub unit_cost: Option<Decimal>,
    pub discount_rate: Option<Decimal>,
    pub delivery_date: Option<NaiveDate>,
}

/// 更新报价单请求
#[derive(Debug, Clone, Default)]
pub struct UpdateQuotationReq {
    pub customer_id: Option<i64>,
    pub contact_id: Option<i64>,
    pub sales_rep_id: Option<i64>,
    pub valid_until: Option<NaiveDate>,
    pub payment_terms: Option<String>,
    pub delivery_terms: Option<String>,
    pub remark: Option<String>,
    pub items: Option<Vec<CreateQuotationItemReq>>,
}

/// 报价单查询过滤
#[derive(Debug, Clone, Default)]
pub struct QuotationQuery {
    pub customer_id: Option<i64>,
    pub status: Option<QuotationStatus>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub keyword: Option<String>,
}

/// 报价单创建参数（repo 层使用）
pub struct CreateQuotationParams<'a> {
    pub doc_number: &'a str,
    pub req: &'a CreateQuotationReq,
    pub sales_rep_id: i64,
    pub total_amount: Decimal,
    pub total_cost: Decimal,
    pub estimated_margin: Decimal,
    pub operator_id: i64,
}

/// 明细行批量插入输入
pub struct QuotationItemInput {
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
