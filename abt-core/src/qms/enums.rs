//! QMS 模块枚举定义
//!
//! 所有枚举存储为 SMALLINT (i16)，应用层强制类型安全。
//! QMS 定义自己的枚举宏（MES 的 define_mes_enum! 无 #[macro_export]，仅模块内可见）。

use std::fmt;

macro_rules! define_qms_enum {
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

// -- Inspection Type --
define_qms_enum!(InspectionType {
    Iqc = 1,
    Ipqc = 2,
    Fqc = 3,
    Oqc = 4,
});

// -- Inspection Source Type --
define_qms_enum!(InspectionSourceType {
    ArrivalNotice = 1,
    WorkOrderRouting = 2,
    ShippingRequest = 3,
    OutsourcingOrder = 4,
});

// -- Inspection Result Type --
define_qms_enum!(InspectionResultType {
    Pass = 1,
    Fail = 2,
    Conditional = 3,
});

// -- Inspection Status --
define_qms_enum!(InspectionStatus {
    Pending = 1,
    Completed = 2,
    Dispositioned = 3,
});

// -- Quality Gate Status --
define_qms_enum!(QualityGateStatus {
    Passed = 1,
    Failed = 2,
    NotRequired = 3,
});

// -- Spec Status --
define_qms_enum!(SpecStatus {
    Draft = 1,
    Active = 2,
    Inactive = 3,
});

// -- MRB Disposition --
define_qms_enum!(MRBDisposition {
    Scrap = 1,
    Return = 2,
    Degrade = 3,
    Rework = 4,
});

// -- Responsible Party --
define_qms_enum!(ResponsibleParty {
    Internal = 1,
    Supplier = 2,
    Customer = 3,
});

// -- MRB Status --
define_qms_enum!(MRBStatus {
    Draft = 1,
    UnderReview = 2,
    Approved = 3,
    Completed = 4,
});

// -- Severity --
define_qms_enum!(Severity {
    Minor = 1,
    Major = 2,
    Critical = 3,
});

// -- RMA Status --
define_qms_enum!(RMAStatus {
    Reported = 1,
    Investigating = 2,
    ActionTaken = 3,
    Closed = 4,
});

// -- InspectionSourceType ↔ DocumentType mapping --

impl InspectionSourceType {
    pub fn from_document_type(dt: crate::shared::enums::document_type::DocumentType) -> Option<Self> {
        use crate::shared::enums::document_type::DocumentType;
        match dt {
            DocumentType::ArrivalNotice => Some(Self::ArrivalNotice),
            DocumentType::WorkReport => Some(Self::WorkOrderRouting),
            DocumentType::ShippingRequest => Some(Self::ShippingRequest),
            DocumentType::OutsourcingOrder => Some(Self::OutsourcingOrder),
            _ => None,
        }
    }

    pub fn to_document_type(self) -> crate::shared::enums::document_type::DocumentType {
        use crate::shared::enums::document_type::DocumentType;
        match self {
            Self::ArrivalNotice => DocumentType::ArrivalNotice,
            Self::WorkOrderRouting => DocumentType::WorkReport,
            Self::ShippingRequest => DocumentType::ShippingRequest,
            Self::OutsourcingOrder => DocumentType::OutsourcingOrder,
        }
    }
}
