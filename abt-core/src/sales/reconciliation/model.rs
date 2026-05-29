use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 对账状态：5 states per 01-sales.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ReconciliationStatus {
    Draft = 1,
    Sent = 2,
    Confirmed = 3,
    Disputed = 4,
    Settled = 5,
}

impl ReconciliationStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Sent),
            3 => Some(Self::Confirmed),
            4 => Some(Self::Disputed),
            5 => Some(Self::Settled),
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
            Self::Confirmed => "Confirmed",
            Self::Disputed => "Disputed",
            Self::Settled => "Settled",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for ReconciliationStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for ReconciliationStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ReconciliationStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ReconciliationStatus: {v}").into())
    }
}

impl Serialize for ReconciliationStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for ReconciliationStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ReconciliationStatus: {v}")))
    }
}

/// 对账单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Reconciliation {
    pub id: i64,
    pub doc_number: String,
    pub customer_id: i64,
    pub period: String,
    pub status: ReconciliationStatus,
    pub total_amount: Decimal,
    pub confirmed_amount: Decimal,
    pub difference: Decimal,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 对账明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReconciliationItem {
    pub id: i64,
    pub reconciliation_id: i64,
    pub shipping_request_id: i64,
    pub sales_order_id: i64,
    pub product_id: i64,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub confirmed: bool,
    pub remark: Option<String>,
}

/// 对账查询过滤
#[derive(Debug, Clone, Default)]
pub struct ReconciliationQuery {
    pub customer_id: Option<i64>,
    pub period: Option<String>,
    pub status: Option<ReconciliationStatus>,
    pub keyword: Option<String>,
}

/// 对账单创建参数（repo 层使用）
pub struct CreateReconciliationParams<'a> {
    pub doc_number: &'a str,
    pub customer_id: i64,
    pub period: &'a str,
    pub total_amount: Decimal,
    pub remark: &'a str,
    pub operator_id: i64,
}

/// 明细行聚合输入（由 repo 层查询填充）
pub struct ReconciliationItemInput {
    pub shipping_request_id: i64,
    pub sales_order_id: i64,
    pub product_id: i64,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}

/// 对账预览项（从已发货数据聚合，用于创建前预览）
pub struct ReconciliationPreviewItem {
    pub shipping_request_id: i64,
    pub sales_order_id: i64,
    pub product_id: i64,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}
