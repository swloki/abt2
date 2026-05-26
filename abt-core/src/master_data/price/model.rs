use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// 价格类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PriceType {
    Purchase = 1,
    Sales = 2,
    StandardCost = 3,
}

impl PriceType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Purchase),
            2 => Some(Self::Sales),
            3 => Some(Self::StandardCost),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for PriceType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for PriceType {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for PriceType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown PriceType: {v}").into())
    }
}

impl serde::Serialize for PriceType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for PriceType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown PriceType: {v}")))
    }
}

/// 价格日志条目
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PriceLogEntry {
    pub log_id: i64,
    pub product_id: i64,
    pub price_type: PriceType,
    pub old_price: Option<Decimal>,
    pub new_price: Decimal,
    pub operator_id: Option<i64>,
    pub remark: String,
    pub created_at: DateTime<Utc>,
}

/// 价格查询过滤
#[derive(Debug, Clone, Default)]
pub struct PriceQuery {
    pub product_id: Option<i64>,
    pub price_type: Option<PriceType>,
}
