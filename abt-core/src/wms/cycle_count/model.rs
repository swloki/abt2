use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::CycleCountStatus;

/// 盘点单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CycleCount {
    pub id: i64,
    pub doc_number: String,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub count_date: NaiveDate,
    pub status: CycleCountStatus,
    pub is_blind: bool,
    pub remark: Option<String>,
    pub operator_id: i64,
    /// 盘点差异金额 = Σ |variance_qty| × unit_cost（complete 时计算）
    #[sqlx(default)]
    pub variance_amount: Decimal,
    /// 审批人（差异超阈值进入 PendingReview 后）
    #[sqlx(default)]
    pub reviewer_id: Option<i64>,
    /// 审批时间
    #[sqlx(default)]
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 列表查询时通过子查询填充的物料项数
    #[sqlx(default)]
    pub item_count: Option<i64>,
}

/// 盘点单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CycleCountItem {
    pub id: i64,
    pub count_id: i64,
    pub bin_id: i64,
    pub product_id: i64,
    pub batch_no: Option<String>,
    pub system_qty: Decimal,
    pub counted_qty: Decimal,
    pub variance_qty: Decimal,
    pub variance_reason: Option<String>,
    pub is_adjusted: bool,
}

/// 创建盘点单请求
#[derive(Debug, Clone)]
pub struct CreateCycleCountReq {
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub count_date: NaiveDate,
    pub is_blind: bool,
    pub remark: Option<String>,
    pub items: Vec<CreateCycleCountItemReq>,
}

/// 创建盘点单明细请求
#[derive(Debug, Clone)]
pub struct CreateCycleCountItemReq {
    pub bin_id: i64,
    pub product_id: i64,
    pub batch_no: Option<String>,
    pub system_qty: Decimal,
}

/// 盘点录入请求
#[derive(Debug, Clone)]
pub struct CountCycleCountReq {
    pub id: i64,
    pub items: Vec<CountItemReq>,
}

/// 盘点明细录入请求
#[derive(Debug, Clone)]
pub struct CountItemReq {
    pub item_id: i64,
    pub counted_qty: Decimal,
    pub variance_reason: Option<String>,
}

/// 盘点单查询过滤
#[derive(Debug, Clone, Default)]
pub struct CycleCountFilter {
    pub status: Option<CycleCountStatus>,
    pub warehouse_id: Option<i64>,
}
