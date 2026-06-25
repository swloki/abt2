use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 拣货单状态：Draft → Picked / Cancelled
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum PickListStatus {
    Draft = 1,
    Picked = 2,
    Cancelled = 3,
}

impl PickListStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Picked),
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
            Self::Picked => "Picked",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for PickListStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for PickListStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for PickListStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown PickListStatus: {v}").into())
    }
}

impl Serialize for PickListStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for PickListStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown PickListStatus: {v}")))
    }
}

/// 拣货单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PickList {
    pub id: i64,
    pub doc_number: String,
    pub outbound_id: i64,
    pub status: PickListStatus,
    pub picker_id: Option<i64>,
    pub picked_at: Option<DateTime<Utc>>,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 拣货单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PickListItem {
    pub id: i64,
    pub pick_list_id: i64,
    pub line_no: i32,
    pub outbound_item_id: i64,
    pub product_id: i64,
    pub warehouse_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub requested_qty: Decimal,
    pub picked_qty: Decimal,
    pub created_at: DateTime<Utc>,
}

/// 拣货单查询过滤
#[derive(Debug, Clone, Default)]
pub struct PickListQuery {
    pub outbound_id: Option<i64>,
    pub status: Option<PickListStatus>,
    pub keyword: Option<String>,
}

/// 明细行插入输入（repo 层）
pub struct PickListItemInput {
    pub line_no: i32,
    pub outbound_item_id: i64,
    pub product_id: i64,
    pub warehouse_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub requested_qty: Decimal,
    pub picked_qty: Decimal,
}

/// 拣货明细录入（人工拣货：picked_qty / warehouse_id / bin_id）。Doc Hub 拣货 drawer 提交。
#[derive(Debug, Clone)]
pub struct PickItemInput {
    pub pick_list_item_id: i64,
    pub picked_qty: Decimal,
    pub warehouse_id: Option<i64>,
    pub bin_id: Option<i64>,
}

/// 创建拣货单参数（repo 层）
pub struct CreatePickListParams<'a> {
    pub doc_number: &'a str,
    pub outbound_id: i64,
    pub picker_id: Option<i64>,
    pub remark: &'a str,
    pub operator_id: i64,
}
