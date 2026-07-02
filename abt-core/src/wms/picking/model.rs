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
    /// 工序（领料用）
    pub operation_id: Option<i64>,
    /// 生产批次（MES 工序级领料用，migration 081）
    #[sqlx(default)]
    pub batch_id: Option<i64>,
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
    pub batch_id: Option<i64>,
    pub source_item_id: Option<i64>,
    pub remark: Option<String>,
}

/// 完成作业单据时的行级实绩（通用 done）
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

// ── 领料专用请求（从 material_requisition 迁入，字段保持兼容调用方）──

/// 手动创建领料单请求（非工单驱动）
#[derive(Debug, Clone)]
pub struct CreateManualReq {
    pub warehouse_id: i64,
    pub requisition_date: NaiveDate,
    pub remark: Option<String>,
    pub items: Vec<CreateManualItemReq>,
}

/// 手动创建领料单行项目请求
#[derive(Debug, Clone)]
pub struct CreateManualItemReq {
    pub product_id: i64,
    pub requested_qty: Decimal,
}

/// 发料请求（整单）
#[derive(Debug, Clone)]
pub struct IssueMaterialReq {
    pub id: i64,
    pub items: Vec<IssueItemReq>,
}

/// 发料请求（行项目）—— item_id 为 stock_picking_items.id，issued_qty 为本次发料量
#[derive(Debug, Clone)]
pub struct IssueItemReq {
    pub item_id: i64,
    pub issued_qty: Decimal,
    pub bin_id: Option<i64>,
}

/// 退料请求 —— requisition_id 语义为 picking_id（保留字段名兼容调用方）
#[derive(Debug, Clone)]
pub struct ReturnMaterialReq {
    pub requisition_id: i64,
    pub items: Vec<ReturnItemReq>,
    pub reason: String,
}

/// 退料行项目请求
#[derive(Debug, Clone)]
pub struct ReturnItemReq {
    pub item_id: i64,
    pub return_qty: Decimal,
    pub bin_id: Option<i64>,
}
