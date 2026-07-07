//! 单个采购订单导出（订单头 + 明细行），供采购作业中心详情 drawer 导出。

use anyhow::Result;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_xlsxwriter::Workbook;
use sqlx::PgPool;

use crate::purchase::enums::PurchaseOrderStatus;

const DETAIL_HEADERS: [&str; 7] = [
    "行号", "物料编码", "物料名称", "数量", "单价", "金额", "已收数量",
];

#[derive(sqlx::FromRow)]
struct PoHeader {
    doc_number: String,
    supplier_name: Option<String>,
    order_date: NaiveDate,
    expected_delivery_date: Option<NaiveDate>,
    status: i16,
    total_amount: Decimal,
    payment_terms: Option<String>,
    delivery_address: Option<String>,
    remark: String,
    operator_name: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PoItemRow {
    line_no: i32,
    product_code: Option<String>,
    product_name: Option<String>,
    quantity: Decimal,
    unit_price: Decimal,
    amount: Decimal,
    received_qty: Decimal,
}

/// 单个采购订单 Excel 导出器：上半部订单头（label: value 两列），下半部明细表。
pub struct PurchaseOrderExporter {
    pool: PgPool,
    order_id: i64,
}

impl PurchaseOrderExporter {
    pub fn new(pool: PgPool, order_id: i64) -> Self {
        Self { pool, order_id }
    }

    pub async fn export(&self) -> Result<Vec<u8>> {
        let mut conn = self.pool.acquire().await?;
        let h = sqlx::query_as::<_, PoHeader>(
            r#"
            SELECT po.doc_number,
                   s.supplier_name AS supplier_name,
                   po.order_date,
                   po.expected_delivery_date,
                   po.status,
                   po.total_amount,
                   po.payment_terms,
                   po.delivery_address,
                   po.remark,
                   u.display_name AS operator_name
            FROM purchase_orders po
            LEFT JOIN suppliers s ON s.supplier_id = po.supplier_id AND s.deleted_at IS NULL
            LEFT JOIN users u ON u.user_id = po.operator_id
            WHERE po.id = $1 AND po.deleted_at IS NULL
            "#,
        )
        .bind(self.order_id)
        .fetch_one(&mut *conn)
        .await?;

        let items = sqlx::query_as::<_, PoItemRow>(
            r#"
            SELECT poi.line_no,
                   p.product_code AS product_code,
                   p.pdt_name AS product_name,
                   poi.quantity,
                   poi.unit_price,
                   poi.amount,
                   poi.received_qty
            FROM purchase_order_items poi
            LEFT JOIN products p ON p.product_id = poi.product_id AND p.deleted_at IS NULL
            WHERE poi.order_id = $1
            ORDER BY poi.line_no
            "#,
        )
        .bind(self.order_id)
        .fetch_all(&mut *conn)
        .await?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // 订单头：A 列 label / B 列 value，每字段一行
        let header_fields: Vec<(&str, String)> = vec![
            ("采购单号", h.doc_number.clone()),
            ("供应商", h.supplier_name.unwrap_or_default()),
            ("订单日期", h.order_date.format("%Y-%m-%d").to_string()),
            ("预计交期", h
                .expected_delivery_date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default()),
            ("状态", po_status_text(h.status).to_string()),
            ("采购员", h.operator_name.unwrap_or_default()),
            ("总金额", h.total_amount.to_string()),
            ("付款条款", h.payment_terms.unwrap_or_default()),
            ("交货地址", h.delivery_address.unwrap_or_default()),
            ("备注", h.remark),
        ];
        let mut row: u32 = 0;
        for (label, value) in &header_fields {
            worksheet.write_string(row, 0, *label)?;
            worksheet.write_string(row, 1, value)?;
            row += 1;
        }

        // 空一行后写明细表
        row += 1;
        for (col, header) in DETAIL_HEADERS.iter().enumerate() {
            worksheet.write_string(row, col as u16, *header)?;
        }
        row += 1;
        for it in items.iter() {
            worksheet.write_number(row, 0, it.line_no as f64)?;
            worksheet.write_string(row, 1, it.product_code.as_deref().unwrap_or(""))?;
            worksheet.write_string(row, 2, it.product_name.as_deref().unwrap_or(""))?;
            worksheet.write_string(row, 3, it.quantity.to_string())?;
            worksheet.write_string(row, 4, it.unit_price.to_string())?;
            worksheet.write_string(row, 5, it.amount.to_string())?;
            worksheet.write_string(row, 6, it.received_qty.to_string())?;
            row += 1;
        }

        Ok(workbook.save_to_buffer()?)
    }
}

/// PO 状态 smallint → 中文（文案对齐 `purchase_work_center::po_status_pill`）
fn po_status_text(v: i16) -> &'static str {
    match PurchaseOrderStatus::from_i16(v) {
        Some(PurchaseOrderStatus::Draft) => "草稿",
        Some(PurchaseOrderStatus::PendingApproval) => "待审批",
        Some(PurchaseOrderStatus::Confirmed) => "待收货",
        Some(PurchaseOrderStatus::PartiallyReceived) => "部分收货",
        Some(PurchaseOrderStatus::Received) => "已收货",
        Some(PurchaseOrderStatus::Closed) => "已关闭",
        Some(PurchaseOrderStatus::Cancelled) => "已取消",
        None => "未知",
    }
}
