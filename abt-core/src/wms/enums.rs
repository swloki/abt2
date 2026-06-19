//! WMS 模块枚举定义
//!
//! 所有枚举存储为 SMALLINT (i16)，应用层强制类型安全。
//! 遵循 shared/enums 的约定：#[repr(i16)] + sqlx Type/Encode/Decode + serde。

/// 为 WMS 枚举生成 sqlx (smallint) + serde (i16) 的 boilerplate
macro_rules! define_wms_enum {
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
    };
}

// -- Warehouse --
define_wms_enum!(WarehouseType {
    RawMaterial = 1,
    FinishedGoods = 2,
    SemiFinished = 3,
    Consumable = 4,
    VirtualOutsource = 5,
});

define_wms_enum!(WarehouseStatus {
    Active = 1,
    Inactive = 2,
});

// -- Zone --
define_wms_enum!(ZoneType {
    Receiving = 1,
    Storage = 2,
    Picking = 3,
    Packing = 4,
    Inspection = 5,
    Returns = 6,
});

// -- Bin --
define_wms_enum!(BinStatus {
    Empty = 1,
    Occupied = 2,
    Locked = 3,
    Disabled = 4,
});

// -- Arrival Notice --
define_wms_enum!(ArrivalStatus {
    Draft = 1,
    Received = 2,
    Inspecting = 3,
    Accepted = 4,
    PartiallyAccepted = 5,
    Rejected = 6,
    Cancelled = 7,
});

// -- Inventory Transaction --
define_wms_enum!(TransactionType {
    PurchaseReceipt = 1,
    ProductionReceipt = 2,
    SalesShipment = 3,
    MaterialIssue = 4,
    MaterialReturn = 5,
    Backflush = 6,
    Transfer = 7,
    FormConversion = 8,
    Adjustment = 9,
    Lock = 10,
    Unlock = 11,
    Scrap = 12,
});

impl TransactionType {
    /// Parse from variant name string (e.g., "PurchaseReceipt")
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "PurchaseReceipt" => Some(Self::PurchaseReceipt),
            "ProductionReceipt" => Some(Self::ProductionReceipt),
            "SalesShipment" => Some(Self::SalesShipment),
            "MaterialIssue" => Some(Self::MaterialIssue),
            "MaterialReturn" => Some(Self::MaterialReturn),
            "Backflush" => Some(Self::Backflush),
            "Transfer" => Some(Self::Transfer),
            "FormConversion" => Some(Self::FormConversion),
            "Adjustment" => Some(Self::Adjustment),
            "Lock" => Some(Self::Lock),
            "Unlock" => Some(Self::Unlock),
            "Scrap" => Some(Self::Scrap),
            _ => None,
        }
    }

    /// 单据号前缀（中文 ERP 习惯），用于在缺少外部单号时兜底生成 doc_number
    pub fn doc_prefix(&self) -> &'static str {
        match self {
            Self::PurchaseReceipt => "RK",
            Self::ProductionReceipt => "SCRK",
            Self::SalesShipment => "CK",
            Self::MaterialIssue => "LL",
            Self::MaterialReturn => "TL",
            Self::Backflush => "BC",
            Self::Transfer => "DB",
            Self::FormConversion => "ZH",
            Self::Adjustment => "PD",
            Self::Lock => "SD",
            Self::Unlock => "JS",
            Self::Scrap => "BF",
        }
    }
}

// -- Material Requisition --
define_wms_enum!(RequisitionStatus {
    Draft = 1,
    Confirmed = 2,
    Issued = 3,
    Cancelled = 4,
    PartiallyIssued = 5,
});

// -- Backflush --
define_wms_enum!(BackflushStatus {
    Draft = 1,
    Executed = 2,
    Adjusted = 3,
});

// -- Cycle Count --
// PendingReview = 6: 差异超阈值待审批（Completed → PendingReview → Adjusted）
define_wms_enum!(CycleCountStatus {
    Draft = 1,
    Counting = 2,
    Completed = 3,
    Adjusted = 4,
    Cancelled = 5,
    PendingReview = 6,
});

// -- Transfer --
define_wms_enum!(TransferStatus {
    Draft = 1,
    InTransit = 2,
    Completed = 3,
    Cancelled = 4,
});

// -- Form Conversion --
define_wms_enum!(ConversionDir {
    Consume = 1,
    Produce = 2,
});

define_wms_enum!(ConversionStatus {
    Draft = 1,
    Completed = 2,
    Cancelled = 3,
});

// -- Inventory Lock --
define_wms_enum!(LockStatus {
    Active = 1,
    Released = 2,
    Cancelled = 3,
});

// -- Strategy --
define_wms_enum!(PutawayType {
    SameMerge = 1,
    Nearest = 2,
    FixedBin = 3,
    EmptyFirst = 4,
});

define_wms_enum!(PickType {
    Fifo = 1,
    Fefo = 2,
    ShortestPath = 3,
    FullPallet = 4,
});

// -- Low Stock Alert --
define_wms_enum!(LowStockAlertStatus {
    Active = 1,
    Acknowledged = 2,
});
