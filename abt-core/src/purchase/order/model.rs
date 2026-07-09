use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::{InvoiceStatus, PurchaseOrderStatus};

// ---------------------------------------------------------------------------
// Entity structs
// ---------------------------------------------------------------------------

/// 采购订单主表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseOrder {
    pub id: i64,
    pub doc_number: String,
    pub supplier_id: i64,
    pub order_date: NaiveDate,
    pub expected_delivery_date: Option<NaiveDate>,
    pub status: PurchaseOrderStatus,
    pub total_amount: Decimal,
    pub currency_code: String,
    pub currency_rate: Decimal,
    pub amount_untaxed: Decimal,
    pub amount_tax: Decimal,
    pub amount_total: Decimal,
    pub discount_amount: Decimal,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: String,
    pub payment_schedule_generated: bool,
    pub invoice_status: InvoiceStatus,
    pub per_billed: Decimal,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 采购订单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseOrderItem {
    pub id: i64,
    pub order_id: i64,
    pub line_no: i32,
    pub product_id: i64,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub received_qty: Decimal,
    pub inspected_qty: Decimal,
    pub returned_qty: Decimal,
    pub quotation_item_id: Option<i64>,
    pub expected_delivery_date: Option<NaiveDate>,
    pub discount_pct: Decimal,
    pub tax_rate_id: Option<i64>,
    pub price_subtotal: Decimal,
    pub price_tax: Decimal,
    pub price_total: Decimal,
    pub qty_invoiced: Decimal,
    pub invoice_status: InvoiceStatus,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

/// 采购订单查询条件
#[derive(Debug, Clone, Default)]
pub struct PurchaseOrderQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<PurchaseOrderStatus>,
    /// 多状态 OR 查询（优先于 `status`）。采购作业中心「在途订单」= [Confirmed, PartiallyReceived]。
    pub statuses: Option<Vec<PurchaseOrderStatus>>,
    pub order_date_start: Option<NaiveDate>,
    pub order_date_end: Option<NaiveDate>,
    /// 单号模糊匹配（ILIKE '%kw%'）
    pub doc_number: Option<String>,
    /// 产品编码反查：匹配明细中含该编码产品的订单（EXISTS join items+products）
    pub product_code: Option<String>,
}

// ---------------------------------------------------------------------------
// Create request structs
// ---------------------------------------------------------------------------

/// 创建采购订单请求
pub struct CreatePurchaseOrderRequest {
    pub supplier_id: i64,
    pub order_date: NaiveDate,
    pub expected_delivery_date: Option<NaiveDate>,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: String,
    pub currency_code: String,
    pub currency_rate: Decimal,
    pub discount_amount: Decimal,
    pub items: Vec<CreateOrderItemRequest>,
}

/// 创建订单明细请求
#[derive(Debug, Clone)]
pub struct CreateOrderItemRequest {
    pub product_id: i64,
    pub line_no: i32,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub quotation_item_id: Option<i64>,
    pub expected_delivery_date: Option<NaiveDate>,
    pub discount_pct: Decimal,
    pub tax_rate_id: Option<i64>,
}

/// 计算单行金额：返回 `(毛额 amount, 折后小计 price_subtotal, 税额 price_tax, 价税合计 price_total)`。
///
/// `rate` 为税率百分数（如 `13` 表示 13%）；无税时传 `Decimal::ZERO`。
pub fn line_amounts(
    quantity: Decimal,
    unit_price: Decimal,
    discount_pct: Decimal,
    rate: Decimal,
) -> (Decimal, Decimal, Decimal, Decimal) {
    let amount = quantity * unit_price;
    let price_subtotal = amount * (Decimal::ONE - discount_pct / Decimal::from(100));
    let price_tax = price_subtotal * rate / Decimal::from(100);
    let price_total = price_subtotal + price_tax;
    (amount, price_subtotal, price_tax, price_total)
}

/// 更新采购订单请求（仅草稿可编辑）
pub struct UpdatePurchaseOrderRequest {
    pub supplier_id: i64,
    pub expected_delivery_date: Option<NaiveDate>,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: String,
    pub currency_code: String,
    pub currency_rate: Decimal,
    pub discount_amount: Decimal,
}

/// 明细变更指令（确认后修改明细用）
#[derive(Debug, Clone)]
pub enum PoItemChange {
    /// 追加新行
    AddItem(CreateOrderItemRequest),
    /// 修改已有行（数量、单价、折扣、税率）
    UpdateItem {
        item_id: i64,
        quantity: Option<Decimal>,
        unit_price: Option<Decimal>,
        discount_pct: Option<Decimal>,
        tax_rate_id: Option<Option<i64>>,
    },
    /// 删除行（仅允许未收货的行）
    RemoveItem { item_id: i64 },
}
