// 应收应付调整模块专属枚举

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

// ---------- AdjustmentDirection ----------
// 调整方向（业务语义，贴合用户「增加/减少」表述）
// - Increase(1): 增加应收 / 应付
// - Decrease(2): 减少应收 / 应付
//
// 过账时按 party_type 映射到台账 LedgerDirection（与 cash_journal 一致）：
//   Customer + Increase → Debit(应收增)   Customer + Decrease → Credit(应收减)
//   Supplier + Increase → Credit(应付增)  Supplier + Decrease → Debit(应付减)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum AdjustmentDirection {
    Increase = 1,
    Decrease = 2,
}

impl AdjustmentDirection {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Increase),
            2 => Some(Self::Decrease),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Increase => "Increase",
            Self::Decrease => "Decrease",
        }
    }
}

impl_sqlx_traits!(AdjustmentDirection);
impl_serde_traits!(AdjustmentDirection);
