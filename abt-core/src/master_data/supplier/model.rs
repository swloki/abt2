use chrono::{DateTime, Utc};

/// Supplier status lifecycle: Prospective -> Qualified -> Probation -> Disqualified / Blacklisted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum SupplierStatus {
    Prospective = 1,
    Qualified = 2,
    Probation = 3,
    Disqualified = 4,
    Blacklisted = 5,
}

impl SupplierStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Prospective),
            2 => Some(Self::Qualified),
            3 => Some(Self::Probation),
            4 => Some(Self::Disqualified),
            5 => Some(Self::Blacklisted),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prospective => "Prospective",
            Self::Qualified => "Qualified",
            Self::Probation => "Probation",
            Self::Disqualified => "Disqualified",
            Self::Blacklisted => "Blacklisted",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for SupplierStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for SupplierStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for SupplierStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown SupplierStatus: {v}").into())
    }
}

impl serde::Serialize for SupplierStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for SupplierStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown SupplierStatus: {v}")))
    }
}

/// Supplier category classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum SupplierCategory {
    Material = 1,
    Service = 2,
    Logistics = 3,
}

impl SupplierCategory {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Material),
            2 => Some(Self::Service),
            3 => Some(Self::Logistics),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for SupplierCategory {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for SupplierCategory {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for SupplierCategory {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown SupplierCategory: {v}").into())
    }
}

impl serde::Serialize for SupplierCategory {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for SupplierCategory {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown SupplierCategory: {v}")))
    }
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// Supplier master entity
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Supplier {
    pub supplier_id: i64,
    pub supplier_code: String,
    pub supplier_name: String,
    pub category: SupplierCategory,
    pub status: SupplierStatus,
    pub tax_number: Option<String>,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Supplier contact person
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SupplierContact {
    pub contact_id: i64,
    pub supplier_id: i64,
    pub contact_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Supplier bank account
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SupplierBankAccount {
    pub account_id: i64,
    pub supplier_id: i64,
    pub bank_name: String,
    pub account_name: String,
    pub account_number: String,
    pub branch: Option<String>,
    pub is_default: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CreateSupplierReq {
    pub supplier_name: String,
    pub category: SupplierCategory,
    pub tax_number: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateSupplierReq {
    pub supplier_name: Option<String>,
    pub category: Option<SupplierCategory>,
    pub status: Option<SupplierStatus>,
    pub tax_number: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SupplierQuery {
    pub name: Option<String>,
    pub status: Option<SupplierStatus>,
    pub category: Option<SupplierCategory>,
}

#[derive(Debug, Clone)]
pub struct CreateContactReq {
    pub contact_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateContactReq {
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CreateBankAccountReq {
    pub bank_name: String,
    pub account_name: String,
    pub account_number: String,
    pub branch: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBankAccountReq {
    pub bank_name: Option<String>,
    pub account_name: Option<String>,
    pub account_number: Option<String>,
    pub branch: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CreateSupplierResult {
    pub id: i64,
    pub warnings: Vec<String>,
}
