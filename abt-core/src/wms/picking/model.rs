use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::wms::enums::{PickingStatus, PickingType};

/// 库存作业单据实体 — 映射 stock_pickings 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StockPicking {
    pub id: i64,
    pub doc_number: String,
    pub picking_type: PickingType,
    pub status: PickingStatus,
    /// 来源单据类型：purchase_order / work_order / sales_order / none
    pub source_type: String,
    pub source_id: Option<i64>,
    /// 客户/供应商（发货/收货用）
    pub partner_id: Option<i64>,
    pub from_warehouse_id: Option<i64>,
    pub from_zone_id: Option<i64>,
    pub from_bin_id: Option<i64>,
    pub to_warehouse_id: Option<i64>,
    pub to_zone_id: Option<i64>,
    pub to_bin_id: Option<i64>,
    pub operator_id: i64,
    pub scheduled_date: Option<NaiveDate>,
    pub done_at: Option<DateTime<Utc>>,
    /// 关联拣货单（发货拣货子流程，决策点 2 方案 A：独立 pick_lists 外键）
    pub pick_list_id: Option<i64>,
    /// 关联工单（领料/生产入库用）
    pub work_order_id: Option<i64>,
    pub remark: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    /// 列表查询时通过子查询填充的明细项数
    #[sqlx(default)]
    pub item_count: Option<i64>,
}

/// 作业单据明细实体 — 映射 stock_picking_items 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StockPickingItem {
    pub id: i64,
    pub picking_id: i64,
    pub product_id: i64,
    pub batch_no: Option<String>,
    pub qty_requested: Decimal,
    pub qty_done: Decimal,
    pub from_bin_id: Option<i64>,
    pub to_bin_id: Option<i64>,
    pub operation_id: Option<i64>,
    pub source_item_id: Option<i64>,
    pub remark: String,
    pub created_at: DateTime<Utc>,
}

/// 创建作业单据请求
#[derive(Debug, Clone)]
pub struct CreatePickingReq {
    pub picking_type: PickingType,
    /// 来源单据类型，默认 "none"
    pub source_type: Option<String>,
    pub source_id: Option<i64>,
    pub partner_id: Option<i64>,
    pub from_warehouse_id: Option<i64>,
    pub from_zone_id: Option<i64>,
    pub from_bin_id: Option<i64>,
    pub to_warehouse_id: Option<i64>,
    pub to_zone_id: Option<i64>,
    pub to_bin_id: Option<i64>,
    pub scheduled_date: Option<NaiveDate>,
    pub work_order_id: Option<i64>,
    pub remark: Option<String>,
    pub items: Vec<CreatePickingItemReq>,
}

/// 创建作业单据明细请求
#[derive(Debug, Clone)]
pub struct CreatePickingItemReq {
    pub product_id: i64,
    pub batch_no: Option<String>,
    pub qty_requested: Decimal,
    pub from_bin_id: Option<i64>,
    pub to_bin_id: Option<i64>,
    pub operation_id: Option<i64>,
    pub source_item_id: Option<i64>,
    pub remark: Option<String>,
}

/// 完成作业单据时的行级实绩（done 按 picking_type 分发：写流水 / 回写来源）
#[derive(Debug, Clone)]
pub struct DoneItemReq {
    /// stock_picking_items.id
    pub item_id: i64,
    pub qty_done: Decimal,
    pub batch_no: Option<String>,
    pub from_bin_id: Option<i64>,
    pub to_bin_id: Option<i64>,
}

/// 作业单据查询过滤
#[derive(Debug, Clone, Default)]
pub struct PickingFilter {
    pub doc_number: Option<String>,
    pub picking_type: Option<PickingType>,
    pub status: Option<PickingStatus>,
    pub source_type: Option<String>,
    pub source_id: Option<i64>,
    pub work_order_id: Option<i64>,
}
