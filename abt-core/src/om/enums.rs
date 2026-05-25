use serde::{Deserialize, Serialize};

// ---------- Boilerplate macros ----------

macro_rules! impl_sqlx_traits {
    ($name:ident) => {
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                sqlx::postgres::PgTypeInfo::with_name("smallint")
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

// ---------- OutsourcingType ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum OutsourcingType {
    Full = 1,
    Process = 2,
    Material = 3,
    Rework = 4,
}

impl OutsourcingType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Full),
            2 => Some(Self::Process),
            3 => Some(Self::Material),
            4 => Some(Self::Rework),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Process => "Process",
            Self::Material => "Material",
            Self::Rework => "Rework",
        }
    }
}

impl_sqlx_traits!(OutsourcingType);
impl_serde_traits!(OutsourcingType);

// ---------- OutsourcingStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum OutsourcingStatus {
    Draft = 1,
    Sent = 2,
    InProduction = 3,
    Delivered = 4,
    Received = 5,
    Closed = 6,
    ConvertedToInternal = 7,
    Cancelled = 8,
}

impl OutsourcingStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Sent),
            3 => Some(Self::InProduction),
            4 => Some(Self::Delivered),
            5 => Some(Self::Received),
            6 => Some(Self::Closed),
            7 => Some(Self::ConvertedToInternal),
            8 => Some(Self::Cancelled),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Sent => "Sent",
            Self::InProduction => "InProduction",
            Self::Delivered => "Delivered",
            Self::Received => "Received",
            Self::Closed => "Closed",
            Self::ConvertedToInternal => "ConvertedToInternal",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(OutsourcingStatus);
impl_serde_traits!(OutsourcingStatus);

// ---------- TrackingNodeType ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum TrackingNodeType {
    SendMaterial = 0,
    CarrierPickup = 1,
    SupplierReceived = 2,
    InProduction = 3,
    Shipped = 4,
    IqcInspected = 5,
    Warehoused = 6,
}

impl TrackingNodeType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            0 => Some(Self::SendMaterial),
            1 => Some(Self::CarrierPickup),
            2 => Some(Self::SupplierReceived),
            3 => Some(Self::InProduction),
            4 => Some(Self::Shipped),
            5 => Some(Self::IqcInspected),
            6 => Some(Self::Warehoused),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SendMaterial => "SendMaterial",
            Self::CarrierPickup => "CarrierPickup",
            Self::SupplierReceived => "SupplierReceived",
            Self::InProduction => "InProduction",
            Self::Shipped => "Shipped",
            Self::IqcInspected => "IqcInspected",
            Self::Warehoused => "Warehoused",
        }
    }
    /// 节点序号，用于顺序校验
    pub fn ordinal(self) -> i16 {
        self as i16
    }
}

impl_sqlx_traits!(TrackingNodeType);
impl_serde_traits!(TrackingNodeType);
