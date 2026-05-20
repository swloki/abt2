//! 采购订单数据模型
//!
//! 包含采购订单实体及其行项目、查询参数等关联结构。

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 采购订单实体
#[derive(Debug, Serialize, Deserialize, Clone, Default, FromRow)]
pub struct PurchaseOrder {
    pub po_id: i64,
    pub po_no: String,
    pub supplier_id: i64,
    /// 1=生产采购, 2=零星采购
    pub order_type: i16,
    /// 1=草稿, 2=已提交, 3=已审核, 4=部分收货, 5=全部收货, 6=已对账, 7=已关闭
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 采购订单行项目
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchaseOrderItem {
    pub item_id: i64,
    pub po_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub received_qty: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 采购订单列表查询结果（含供应商名称）
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchaseOrderDetail {
    pub po_id: i64,
    pub po_no: String,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub order_type: i16,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 采购订单查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PurchaseOrderQuery {
    /// 关键词（模糊匹配 po_no）
    pub keyword: Option<String>,
    /// 供应商过滤
    pub supplier_id: Option<i64>,
    /// 订单类型过滤
    pub order_type: Option<i16>,
    /// 状态过滤
    pub status: Option<i16>,
    /// 页码
    pub page: Option<i64>,
    /// 每页数量
    pub page_size: Option<i64>,
}

/// 采购订单详情（含行项目）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurchaseOrderWithItems {
    #[serde(flatten)]
    pub order: PurchaseOrder,
    pub items: Vec<PurchaseOrderItem>,
}

/// 创建/更新采购订单的行项目输入
#[derive(Debug, Clone)]
pub struct PurchaseOrderItemInput {
    pub product_id: i64,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub remark: Option<String>,
}
