// AR/AP 模块专属枚举

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

// ---------- LedgerDirection ----------
// 台账方向：表示该笔交易对 AR/AP 余额的影响
// - Debit(1): 应收增加（借）/ 应付减少（借）
// - Credit(2): 应收减少（贷）/ 应付增加（贷）

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum LedgerDirection {
    Debit = 1,
    Credit = 2,
}

impl LedgerDirection {
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
            Self::Debit => "Debit",
            Self::Credit => "Credit",
        }
    }
}

impl_sqlx_traits!(LedgerDirection);
impl_serde_traits!(LedgerDirection);

// ---------- AgeingBasis ----------
// 账龄分析基准

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum AgeingBasis {
    DueDate = 1,
    TransactionDate = 2,
}

impl AgeingBasis {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::DueDate),
            2 => Some(Self::TransactionDate),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DueDate => "DueDate",
            Self::TransactionDate => "TransactionDate",
        }
    }
}

impl_sqlx_traits!(AgeingBasis);
impl_serde_traits!(AgeingBasis);
