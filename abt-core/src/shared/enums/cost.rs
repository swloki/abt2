#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum CostType {
    Material = 1,
    Labor = 2,
    Overhead = 3,
    Outsource = 4,
    Rework = 5,
    Scrap = 6,
}

impl CostType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Material), 2 => Some(Self::Labor),
            3 => Some(Self::Overhead), 4 => Some(Self::Outsource),
            5 => Some(Self::Rework), 6 => Some(Self::Scrap), _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum CostEntityType {
    Product = 1,
    WorkOrder = 2,
    SalesOrder = 3,
    PurchaseOrder = 4,
    Inspection = 5,
}

impl CostEntityType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Product), 2 => Some(Self::WorkOrder),
            3 => Some(Self::SalesOrder), 4 => Some(Self::PurchaseOrder),
            5 => Some(Self::Inspection), _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

impl sqlx::Type<sqlx::Postgres> for CostType {
    fn type_info() -> sqlx::postgres::PgTypeInfo { sqlx::postgres::PgTypeInfo::with_name("smallint") }
}
impl sqlx::Encode<'_, sqlx::Postgres> for CostType {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}
impl sqlx::Decode<'_, sqlx::Postgres> for CostType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown CostType: {v}").into())
    }
}

impl sqlx::Type<sqlx::Postgres> for CostEntityType {
    fn type_info() -> sqlx::postgres::PgTypeInfo { sqlx::postgres::PgTypeInfo::with_name("smallint") }
}
impl sqlx::Encode<'_, sqlx::Postgres> for CostEntityType {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}
impl sqlx::Decode<'_, sqlx::Postgres> for CostEntityType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown CostEntityType: {v}").into())
    }
}

impl serde::Serialize for CostType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}
impl<'de> serde::Deserialize<'de> for CostType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown CostType: {v}")))
    }
}

impl serde::Serialize for CostEntityType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}
impl<'de> serde::Deserialize<'de> for CostEntityType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown CostEntityType: {v}")))
    }
}
