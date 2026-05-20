//! 报价单数据模型
//!
//! 包含报价单主表及行项目的实体定义和查询参数。

use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 报价单主表
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Quotation {
    pub quotation_id: i64,
    pub quotation_no: String,
    pub customer_name: String,
    pub contact_person: Option<String>,
    pub contact_phone: Option<String>,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub valid_until: Option<NaiveDateTime>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
    pub items: Vec<QuotationItem>,
}

impl<'r> FromRow<'r, PgRow> for Quotation {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Quotation {
            quotation_id: row.try_get("quotation_id")?,
            quotation_no: row.try_get("quotation_no")?,
            customer_name: row.try_get("customer_name")?,
            contact_person: row.try_get("contact_person").ok(),
            contact_phone: row.try_get("contact_phone").ok(),
            status: row.try_get("status")?,
            total_amount: row.try_get("total_amount")?,
            remark: row.try_get("remark").ok(),
            valid_until: row.try_get("valid_until").ok(),
            operator_id: row.try_get("operator_id").ok(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at").ok(),
            items: Vec::new(), // Filled separately via secondary query
        })
    }
}

/// 报价单行项目
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuotationItem {
    pub item_id: i64,
    pub quotation_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub discount: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<String>,
    pub created_at: NaiveDateTime,
}

impl<'r> FromRow<'r, PgRow> for QuotationItem {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(QuotationItem {
            item_id: row.try_get("item_id")?,
            quotation_id: row.try_get("quotation_id")?,
            product_id: row.try_get("product_id")?,
            product_code: row.try_get("product_code").ok(),
            product_name: row.try_get("product_name").ok(),
            unit: row.try_get("unit").ok(),
            unit_price: row.try_get("unit_price")?,
            quantity: row.try_get("quantity")?,
            discount: row.try_get("discount")?,
            subtotal: row.try_get("subtotal")?,
            remark: row.try_get("remark").ok(),
            created_at: row.try_get("created_at")?,
        })
    }
}

/// 报价单查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct QuotationQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
