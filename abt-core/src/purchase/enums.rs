// 采购模块专属枚举 — 全部 `#[repr(i16)]`

use serde::{Deserialize, Serialize};

// ---------- Boilerplate macros ----------

macro_rules! impl_sqlx_traits {
    ($name:ident) => {
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl sqlx::Encode<'_, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
            }
        }

        impl sqlx::Decode<'_, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'_>,
            ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
                Self::from_i16(v).ok_or_else(|| {
                    let msg = format!("unknown {}: {v}", stringify!($name));
                    msg.into()
                })
            }
        }
    };
}

macro_rules! impl_serde_traits {
    ($name:ident) => {
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_i16(self.as_i16())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = i16::deserialize(d)?;
                Self::from_i16(v).ok_or_else(|| {
                    let msg = format!("unknown {}: {v}", stringify!($name));
                    serde::de::Error::custom(msg)
                })
            }
        }
    };
}

// ---------- PurchaseQuotationStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PurchaseQuotationStatus {
    Draft = 1,
    Active = 2,
    Expired = 3,
    Cancelled = 4,
}

impl PurchaseQuotationStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Active),
            3 => Some(Self::Expired),
            4 => Some(Self::Cancelled),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Active => "Active",
            Self::Expired => "Expired",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(PurchaseQuotationStatus);
impl_serde_traits!(PurchaseQuotationStatus);

// ---------- PurchaseOrderStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PurchaseOrderStatus {
    Draft = 1,
    Confirmed = 2,
    PartiallyReceived = 3,
    Received = 4,
    Closed = 5,
    Cancelled = 6,
}

impl PurchaseOrderStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::PartiallyReceived),
            4 => Some(Self::Received),
            5 => Some(Self::Closed),
            6 => Some(Self::Cancelled),
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
            Self::PartiallyReceived => "PartiallyReceived",
            Self::Received => "Received",
            Self::Closed => "Closed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(PurchaseOrderStatus);
impl_serde_traits!(PurchaseOrderStatus);

// ---------- PurchaseReturnStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PurchaseReturnStatus {
    Draft = 1,
    Confirmed = 2,
    Shipped = 3,
    Settled = 4,
    Cancelled = 5,
}

impl PurchaseReturnStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::Shipped),
            4 => Some(Self::Settled),
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
            Self::Shipped => "Shipped",
            Self::Settled => "Settled",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(PurchaseReturnStatus);
impl_serde_traits!(PurchaseReturnStatus);

// ---------- PurchaseReconStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PurchaseReconStatus {
    Draft = 1,
    Confirmed = 2,
    Settled = 3,
}

impl PurchaseReconStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::Settled),
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
            Self::Settled => "Settled",
        }
    }
}

impl_sqlx_traits!(PurchaseReconStatus);
impl_serde_traits!(PurchaseReconStatus);

// ---------- PaymentStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PaymentStatus {
    Draft = 1,
    Approved = 2,
    Paid = 3,
    Cancelled = 4,
}

impl PaymentStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Approved),
            3 => Some(Self::Paid),
            4 => Some(Self::Cancelled),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Approved => "Approved",
            Self::Paid => "Paid",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(PaymentStatus);
impl_serde_traits!(PaymentStatus);

// ---------- PaymentMethod ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PaymentMethod {
    BankTransfer = 1,
    Cash = 2,
    Note = 3,
}

impl PaymentMethod {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::BankTransfer),
            2 => Some(Self::Cash),
            3 => Some(Self::Note),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl_sqlx_traits!(PaymentMethod);
impl_serde_traits!(PaymentMethod);

// ---------- MiscRequestStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum MiscRequestStatus {
    Draft = 1,
    Approved = 2,
    Purchasing = 3,
    Received = 4,
    Closed = 5,
    Cancelled = 6,
}

impl MiscRequestStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Approved),
            3 => Some(Self::Purchasing),
            4 => Some(Self::Received),
            5 => Some(Self::Closed),
            6 => Some(Self::Cancelled),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Approved => "Approved",
            Self::Purchasing => "Purchasing",
            Self::Received => "Received",
            Self::Closed => "Closed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(MiscRequestStatus);
impl_serde_traits!(MiscRequestStatus);
