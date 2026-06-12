//! 采购需求池 — 请求/响应模型

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

/// 需求查询参数（订单行维度）
#[derive(Debug, Clone, Default)]
pub struct DemandPoolQuery {
    pub status: Option<i16>,       // DemandStatus 枚举值，默认 Pending(1)
    pub product_id: Option<i64>,
    pub order_id: Option<i64>,
}

/// 需求摘要（订单行维度 — 展示给操作员）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DemandSummary {
    pub id: i64,
    pub order_id: i64,
    pub order_no: String,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub demand_status: i16,
    pub created_at: NaiveDateTime,
}

/// 物料聚合查询参数
#[derive(Debug, Clone, Default)]
pub struct MaterialAggQuery {
    pub product_id: Option<i64>,
}

/// 物料聚合摘要（物料维度 — 采购员主要操作视图）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MaterialAggSummary {
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub total_demand_qty: Decimal,
    pub demand_count: i64,
    pub earliest_required_date: Option<NaiveDate>,
    pub latest_required_date: Option<NaiveDate>,
}

/// 从需求创建采购订单请求
#[derive(Debug, Clone)]
pub struct CreateOrderFromDemandsReq {
    pub demand_ids: Vec<i64>,
    pub supplier_id: i64,
    pub expected_delivery_date: Option<NaiveDate>,
    pub remark: String,
}

/// 创建下游单据的统一响应（含部分成功信息）
#[derive(Debug, Clone)]
pub struct CreateDownstreamResult {
    pub doc_id: i64,
    pub processed_demand_count: usize,
    pub skipped_demands: Vec<SkippedDemand>,
    /// "Confirmed" — 前端用此字段判断补货已启动
    pub demand_status: String,
}

/// 被跳过的需求
#[derive(Debug, Clone)]
pub struct SkippedDemand {
    pub demand_id: i64,
    pub reason: String,
}

/// 乐观锁返回的已锁定需求数据
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LockedDemand {
    pub id: i64,
    pub product_id: i64,
    pub source_id: i64,
    pub source_line_id: i64,
    pub acquire_channel: i16,
    pub required_qty: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
}
