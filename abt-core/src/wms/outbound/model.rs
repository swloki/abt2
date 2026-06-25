use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 发货状态：5 states per 01-sales.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ShippingStatus {
    Draft = 1,
    Confirmed = 2,
    Picking = 3,
    Shipped = 4,
    Cancelled = 5,
}

impl ShippingStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::Picking),
            4 => Some(Self::Shipped),
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
            Self::Picking => "Picking",
            Self::Shipped => "Shipped",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for ShippingStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for ShippingStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ShippingStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ShippingStatus: {v}").into())
    }
}

impl Serialize for ShippingStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> Deserialize<'de> for ShippingStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ShippingStatus: {v}")))
    }
}

/// 发货申请实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShippingRequest {
    pub id: i64,
    pub doc_number: String,
    pub order_id: Option<i64>,
    pub customer_id: i64,
    pub request_date: NaiveDate,
    pub expected_ship_date: Option<NaiveDate>,
    pub status: ShippingStatus,
    pub shipping_address: String,
    pub carrier: String,
    pub tracking_number: String,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 发货申请明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShippingRequestItem {
    pub id: i64,
    pub shipping_request_id: i64,
    pub line_no: i32,
    pub order_item_id: i64,
    pub product_id: i64,
    pub warehouse_id: Option<i64>,
    pub requested_qty: Decimal,
    pub shipped_qty: Decimal,
    pub description: String,
}

/// 从订单创建发货请求（正式创建，要求 order_id）
#[derive(Debug, Clone)]
pub struct CreateFromOrderReq {
    pub order_id: i64,
    pub expected_ship_date: Option<NaiveDate>,
    pub shipping_address: Option<String>,
    pub items: Vec<CreateShippingItemReq>,
}

/// 草稿创建请求（宽松校验，仅要求 customer_id）
#[derive(Debug, Clone)]
pub struct CreateDraftReq {
    pub customer_id: i64,
    pub order_id: Option<i64>,
    pub expected_ship_date: Option<NaiveDate>,
    pub shipping_address: Option<String>,
    pub carrier: Option<String>,
    pub remark: Option<String>,
    pub items: Vec<CreateDraftItemReq>,
}

/// 草稿明细行（order_item_id 可选，支持手动添加的行）
#[derive(Debug, Clone)]
pub struct CreateDraftItemReq {
    pub order_item_id: Option<i64>,
    pub product_id: Option<i64>,
    pub warehouse_id: Option<i64>,
    pub requested_qty: Decimal,
    pub description: String,
}

/// 草稿更新请求（全量替换语义）
#[derive(Debug, Clone, Default)]
pub struct UpdateDraftReq {
    pub customer_id: Option<i64>,
    pub order_id: Option<i64>,
    pub expected_ship_date: Option<NaiveDate>,
    pub shipping_address: Option<String>,
    pub carrier: Option<String>,
    pub remark: Option<String>,
    pub items: Option<Vec<CreateDraftItemReq>>,
}

/// 创建发货明细请求（正式创建，从订单关联）
#[derive(Debug, Clone)]
pub struct CreateShippingItemReq {
    pub order_item_id: i64,
    pub warehouse_id: Option<i64>,
    pub requested_qty: Decimal,
}

/// 一键申请发货行（订单详情页弹窗提交，销售不指定仓库；仓库由拣货时确定）
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RequestShippingItemReq {
    pub order_item_id: i64,
    pub requested_qty: Decimal,
}

/// 更新发货申请请求（非草稿状态，仅改基础字段）
#[derive(Debug, Clone, Default)]
pub struct UpdateShippingReq {
    pub expected_ship_date: Option<NaiveDate>,
    pub shipping_address: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub remark: Option<String>,
}

/// 发货查询过滤
#[derive(Debug, Clone, Default)]
pub struct ShippingQuery {
    pub order_id: Option<i64>,
    pub status: Option<ShippingStatus>,
    pub keyword: Option<String>,
    pub customer_id: Option<i64>,
}

/// 发货申请创建参数（repo 层使用）
pub struct CreateShippingRequestParams<'a> {
    pub doc_number: &'a str,
    pub order_id: Option<i64>,
    pub customer_id: i64,
    pub expected_ship_date: Option<NaiveDate>,
    pub shipping_address: &'a str,
    pub carrier: &'a str,
    pub remark: &'a str,
    pub operator_id: i64,
}

/// 明细行批量插入输入
pub struct ShippingItemInput {
    pub line_no: i32,
    pub order_item_id: i64,
    pub product_id: i64,
    pub warehouse_id: Option<i64>,
    pub requested_qty: Decimal,
    pub description: String,
}

/// 发货单 Hub 摘要带数据（首屏轻量查询，含缺货 ATP 判定）
#[derive(Debug, Clone)]
pub struct ShippingHubSummary {
    pub pending_pick_qty: Decimal,        // 待拣 Σ requested_qty
    pub picked_qty: Decimal,              // 已拣 Σ picked_qty（来自 PickList）
    pub shipped_qty: Decimal,             // 已发 Σ shipped_qty
    pub shortage: Option<ShortageSignal>, // 缺货红点；None = 无缺货
}

/// 缺货信号（ATP < 待发量）。product_name 为 MVP 占位，前端可按 product_id 解析真实名。
#[derive(Debug, Clone)]
pub struct ShortageSignal {
    pub product_id: i64,
    pub product_name: String,
    pub requested_qty: Decimal,
    pub available_qty: Decimal, // ATP 口径（InventoryTransactionService::query_available）
}
