//! MES 模块枚举定义
//!
//! 所有枚举存储为 SMALLINT (i16)，应用层强制类型安全。
//! 遵循 shared/enums 的约定：#[repr(i16)] + sqlx Type/Encode/Decode + serde。

use std::fmt;

macro_rules! define_mes_enum {
    ($name:ident { $($variant:ident = $val:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(i16)]
        pub enum $name {
            $($variant = $val),*
        }

        impl $name {
            pub fn from_i16(v: i16) -> Option<Self> {
                match v {
                    $($val => Some(Self::$variant),)*
                    _ => None,
                }
            }
            pub fn as_i16(self) -> i16 { self as i16 }
        }

        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl sqlx::Encode<'_, sqlx::Postgres> for $name {
            fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer)
                -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>>
            {
                <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
            }
        }

        impl sqlx::Decode<'_, sqlx::Postgres> for $name {
            fn decode(value: sqlx::postgres::PgValueRef<'_>)
                -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
            {
                let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
                Self::from_i16(v).ok_or_else(|| format!("unknown {}: {v}", stringify!($name)).into())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_i16(self.as_i16())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = i16::deserialize(d)?;
                Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(
                    format!("unknown {}: {v}", stringify!($name))
                ))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant => write!(f, stringify!($variant)),)*
                }
            }
        }
    };
}

// -- Production Plan --
define_mes_enum!(PlanType {
    Mto = 1,
    Mts = 2,
});

define_mes_enum!(PlanStatus {
    Draft = 1,
    Confirmed = 2,
    InProgress = 3,
    Completed = 4,
    Cancelled = 5,
});

define_mes_enum!(PlanItemStatus {
    Planned = 1,
    Released = 2,
    InProduction = 3,
    Completed = 4,
    Cancelled = 5,
});

// -- Work Order --
define_mes_enum!(WorkOrderStatus {
    Draft = 1,
    Planned = 2,
    Released = 3,
    Closed = 4,
    Cancelled = 5,
});

// -- Production Batch --
define_mes_enum!(BatchStatus {
    Pending = 1,
    InProgress = 2,
    Suspended = 3,
    PendingReceipt = 4,
    Completed = 5,
    Cancelled = 6,
});

// -- Work Order Routing --
define_mes_enum!(RoutingStatus {
    Pending = 1,
    InProgress = 2,
    Completed = 3,
    Skipped = 4,
});

// -- Shift --
define_mes_enum!(ShiftType {
    Day = 1,
    Night = 2,
});

// -- Inspection --
define_mes_enum!(InspectionType {
    FirstArticle = 1,
    InProcess = 2,
    Final = 3,
});

define_mes_enum!(InspectionResultType {
    Pass = 1,
    Fail = 2,
    Conditional = 3,
});

// -- Receipt --
define_mes_enum!(ReceiptStatus {
    Draft = 1,
    Confirmed = 2,
    Cancelled = 3,
});

// -- Defect Reason --
define_mes_enum!(DefectReason {
    MaterialDefect = 1,
    EquipmentFault = 2,
    OperatorError = 3,
    ProcessIssue = 4,
});

impl DefectReason {
    /// MaterialDefect, EquipmentFault, ProcessIssue → true (affects wage)
    /// OperatorError → false (does not affect wage)
    pub fn affect_wage(self) -> bool {
        matches!(self, Self::MaterialDefect | Self::EquipmentFault | Self::ProcessIssue)
    }
}

// -- Production Exception --
define_mes_enum!(ExceptionType {
    BatchSuspended = 1,
    BatchScrapped = 2,
    DefectAnomaly = 3,
    InspectionFailed = 4,
    EquipmentFault = 5,
});

define_mes_enum!(ExceptionStatus {
    Pending = 1,
    Processing = 2,
    Closed = 3,
    ConditionalRelease = 4,
    Resolved = 5,
});

define_mes_enum!(ExceptionSeverity {
    Urgent = 1,
    Normal = 2,
    Low = 3,
});

define_mes_enum!(ReasonCategory {
    MaterialDefect = 1,
    EquipmentFault = 2,
    OperatorError = 3,
    ProcessIssue = 4,
});
