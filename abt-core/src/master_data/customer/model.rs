use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 客户分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum CustomerCategory {
    Distributor = 1,
    DirectCustomer = 2,
    OEM = 3,
    Retailer = 4,
}

impl CustomerCategory {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Distributor),
            2 => Some(Self::DirectCustomer),
            3 => Some(Self::OEM),
            4 => Some(Self::Retailer),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for CustomerCategory {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for CustomerCategory {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for CustomerCategory {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown CustomerCategory: {v}").into())
    }
}

impl Serialize for CustomerCategory {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for CustomerCategory {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown CustomerCategory: {v}")))
    }
}

/// 客户状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum CustomerStatus {
    Prospective = 1,
    Active = 2,
    Inactive = 3,
    Blacklisted = 4,
}

impl CustomerStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Prospective),
            2 => Some(Self::Active),
            3 => Some(Self::Inactive),
            4 => Some(Self::Blacklisted),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prospective => "Prospective",
            Self::Active => "Active",
            Self::Inactive => "Inactive",
            Self::Blacklisted => "Blacklisted",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for CustomerStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for CustomerStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for CustomerStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown CustomerStatus: {v}").into())
    }
}

impl Serialize for CustomerStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for CustomerStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown CustomerStatus: {v}")))
    }
}

/// 客户实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Customer {
    #[sqlx(rename = "customer_id")]
    pub id: i64,
    #[sqlx(rename = "customer_code")]
    pub code: String,
    #[sqlx(rename = "customer_name")]
    pub name: String,
    pub short_name: Option<String>,
    pub category: CustomerCategory,
    pub status: CustomerStatus,
    pub tax_number: Option<String>,
    pub invoice_title: Option<String>,
    pub credit_limit: Option<Decimal>,
    pub payment_terms: Option<String>,
    pub receivable_account: Option<String>,
    pub owner_id: Option<i64>,
    pub department_id: Option<i64>,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 客户联系人实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CustomerContact {
    #[sqlx(rename = "contact_id")]
    pub id: i64,
    pub customer_id: i64,
    #[sqlx(rename = "contact_name")]
    pub name: String,
    pub position: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_primary: bool,
}

/// 客户地址实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CustomerAddress {
    #[sqlx(rename = "address_id")]
    pub id: i64,
    pub customer_id: i64,
    pub address_type: String,
    pub province: String,
    pub city: String,
    pub district: Option<String>,
    pub detail: String,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub is_default: bool,
}

/// 创建客户请求
#[derive(Debug, Clone)]
pub struct CreateCustomerReq {
    pub customer_name: String,
    pub short_name: Option<String>,
    pub category: CustomerCategory,
    pub tax_number: Option<String>,
    pub invoice_title: Option<String>,
    pub credit_limit: Option<Decimal>,
    pub payment_terms: Option<String>,
    pub receivable_account: Option<String>,
    pub remark: Option<String>,
}

/// 更新客户请求
#[derive(Debug, Clone, Default)]
pub struct UpdateCustomerReq {
    pub customer_name: Option<String>,
    pub short_name: Option<String>,
    pub category: Option<CustomerCategory>,
    pub status: Option<CustomerStatus>,
    pub tax_number: Option<String>,
    pub invoice_title: Option<String>,
    pub credit_limit: Option<Decimal>,
    pub payment_terms: Option<String>,
    pub receivable_account: Option<String>,
    pub remark: Option<String>,
}

/// 客户查询过滤
#[derive(Debug, Clone, Default)]
pub struct CustomerQuery {
    pub name: Option<String>,
    pub status: Option<CustomerStatus>,
    pub category: Option<CustomerCategory>,
    pub owner_id: Option<i64>,
}

/// 创建联系人请求
#[derive(Debug, Clone)]
pub struct CreateContactReq {
    pub contact_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: bool,
}

/// 更新联系人请求
#[derive(Debug, Clone, Default)]
pub struct UpdateContactReq {
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: Option<bool>,
}

/// 创建地址请求
#[derive(Debug, Clone)]
pub struct CreateAddressReq {
    pub address_type: String,
    pub province: String,
    pub city: String,
    pub district: Option<String>,
    pub detail: String,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub is_default: bool,
}

/// 更新地址请求
#[derive(Debug, Clone, Default)]
pub struct UpdateAddressReq {
    pub address_type: Option<String>,
    pub province: Option<String>,
    pub city: Option<String>,
    pub district: Option<String>,
    pub detail: Option<String>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub is_default: Option<bool>,
}

