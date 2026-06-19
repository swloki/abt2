// GL 总账模块专属枚举 — 全部 `#[repr(i16)]`

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

// ---------- AccountType ----------
// 科目类型：1资产/2负债/3权益/4收入/5成本/6费用

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum AccountType {
    Asset = 1,
    Liability = 2,
    Equity = 3,
    Revenue = 4,
    Cost = 5,
    Expense = 6,
}

impl AccountType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Asset),
            2 => Some(Self::Liability),
            3 => Some(Self::Equity),
            4 => Some(Self::Revenue),
            5 => Some(Self::Cost),
            6 => Some(Self::Expense),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asset => "资产",
            Self::Liability => "负债",
            Self::Equity => "权益",
            Self::Revenue => "收入",
            Self::Cost => "成本",
            Self::Expense => "费用",
        }
    }
}

impl_sqlx_traits!(AccountType);
impl_serde_traits!(AccountType);

// ---------- BalanceDirection ----------
// 余额方向：1借/2贷

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum BalanceDirection {
    Debit = 1,
    Credit = 2,
}

impl BalanceDirection {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Debit),
            2 => Some(Self::Credit),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debit => "借",
            Self::Credit => "贷",
        }
    }
}

impl_sqlx_traits!(BalanceDirection);
impl_serde_traits!(BalanceDirection);

// ---------- EntryStatus ----------
// 凭证状态：1draft/2posted/3cancelled

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum EntryStatus {
    Draft = 1,
    Posted = 2,
    Cancelled = 3,
}

impl EntryStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Posted),
            3 => Some(Self::Cancelled),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Posted => "Posted",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(EntryStatus);
impl_serde_traits!(EntryStatus);

// ---------- PeriodStatus ----------
// 会计期间状态：1open/2closed

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PeriodStatus {
    Open = 1,
    Closed = 2,
}

impl PeriodStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Open),
            2 => Some(Self::Closed),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Closed => "Closed",
        }
    }
}

impl_sqlx_traits!(PeriodStatus);
impl_serde_traits!(PeriodStatus);
