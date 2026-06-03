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
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
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

/// Supplier category: RawMaterial, Packaging, Outsourcing, Consumable, Service
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum SupplierCategory {
    RawMaterial = 1,
    Packaging = 2,
    Outsourcing = 3,
    Consumable = 4,
    Service = 5,
}

impl SupplierCategory {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::RawMaterial),
            2 => Some(Self::Packaging),
            3 => Some(Self::Outsourcing),
            4 => Some(Self::Consumable),
            5 => Some(Self::Service),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for SupplierCategory {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
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
    #[sqlx(rename = "supplier_id")]
    pub id: i64,
    #[sqlx(rename = "supplier_code")]
    pub code: String,
    #[sqlx(rename = "supplier_name")]
    pub name: String,
    pub short_name: Option<String>,
    pub category: SupplierCategory,
    pub status: SupplierStatus,
    pub tax_number: Option<String>,
    pub lead_time_days: i32,
    pub payment_terms: Option<String>,
    pub remark: String,
    pub currency: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Supplier contact person
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SupplierContact {
    #[sqlx(rename = "contact_id")]
    pub id: i64,
    pub supplier_id: i64,
    #[sqlx(rename = "contact_name")]
    pub name: String,
    pub position: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_primary: bool,
}

/// Supplier bank account
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SupplierBankAccount {
    #[sqlx(rename = "account_id")]
    pub id: i64,
    pub supplier_id: i64,
    pub bank_name: String,
    pub account_name: String,
    pub account_number: String,
    pub is_default: bool,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CreateSupplierReq {
    pub supplier_name: String,
    pub short_name: Option<String>,
    pub category: SupplierCategory,
    pub tax_number: Option<String>,
    pub lead_time_days: Option<i32>,
    pub payment_terms: Option<String>,
    pub remark: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateSupplierReq {
    pub supplier_name: Option<String>,
    pub short_name: Option<String>,
    pub category: Option<SupplierCategory>,
    pub status: Option<SupplierStatus>,
    pub tax_number: Option<String>,
    pub lead_time_days: Option<i32>,
    pub payment_terms: Option<String>,
    pub remark: Option<String>,
    pub currency: Option<String>,
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
    pub is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBankAccountReq {
    pub bank_name: Option<String>,
    pub account_name: Option<String>,
    pub account_number: Option<String>,
    pub is_default: Option<bool>,
}
