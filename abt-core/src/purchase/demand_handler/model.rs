//! 采购需求池 — 请求/响应模型

use chrono::{NaiveDate, DateTime, Utc};
use rust_decimal::Decimal;

/// 需求查询参数（订单行维度）
#[derive(Debug, Clone, Default)]
pub struct DemandPoolQuery {
    pub status: Option<i16>,       // DemandStatus 枚举值，默认 Pending(1)
    pub product_id: Option<i64>,
    pub order_id: Option<i64>,
    pub keyword: Option<String>,              // 模糊搜索物料名称/编码
    pub required_date_start: Option<NaiveDate>, // 日期范围起点
    pub required_date_end: Option<NaiveDate>,   // 日期范围终点
    /// 按供应商过滤（采购明细 tab）：只返回该供应商「有效报价(Active、未过期)」
    /// 所覆盖物料的需求。语义 = "该供应商可供的待采购需求"。
    pub supplier_id: Option<i64>,
}

/// 需求摘要（订单行维度 — 展示给操作员）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DemandSummary {
    pub id: i64,
    pub order_id: i64,
    pub order_no: Option<String>,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: Decimal,
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub demand_status: i16,
    pub target_doc_id: Option<i64>,       // 关联下游单据 ID
    pub target_doc_type: Option<i16>,     // 关联下游单据类型 (7=PO,12=PP,10=WO,11=OM)
    pub cascade_from_product_name: Option<String>, // BOM 级联来源成品名称（Odoo origin 等价）
    pub created_at: DateTime<Utc>,
}
/// 物料汇总视图排序方式
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MaterialSort {
    /// 总需求量降序（默认）
    #[default]
    DemandQty,
    /// 新订单优先：最近确认进来的订单需求排前（MAX(created_at) DESC）
    NewestOrder,
    /// 到期日优先：最早到期排前（MIN(required_date) ASC）
    EarliestDate,
}

impl MaterialSort {
    /// 从 query string 解析（"newest"→NewestOrder, "due"→EarliestDate, 其余→DemandQty）
    pub fn from_query(s: Option<&str>) -> Self {
        match s {
            Some("newest") => Self::NewestOrder,
            Some("due") => Self::EarliestDate,
            _ => Self::DemandQty,
        }
    }

    /// 转 query string 值（默认返回空串，不进 URL，与 date_filter 一致）
    pub fn as_query(self) -> &'static str {
        match self {
            Self::DemandQty => "",
            Self::NewestOrder => "newest",
            Self::EarliestDate => "due",
        }
    }

    /// ORDER BY 子句（白名单字面量，无注入风险）
    pub fn order_by(self) -> &'static str {
        match self {
            Self::DemandQty => "total_demand_qty DESC",
            // SELECT 未取 created_at 裸列，GROUP BY 后必须用聚合表达式
            Self::NewestOrder => "MAX(created_at) DESC NULLS LAST",
            Self::EarliestDate => "MIN(required_date) ASC NULLS LAST",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MaterialAggQuery {
    pub product_id: Option<i64>,
    pub keyword: Option<String>,              // 模糊搜索物料名称/编码
    pub required_date_start: Option<NaiveDate>, // 日期范围起点
    pub required_date_end: Option<NaiveDate>,   // 日期范围终点
    pub sort: MaterialSort,                    // 排序方式（默认总需求量）
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
