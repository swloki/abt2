use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Quotation {
    pub quotation_id: i64,
    pub quotation_no: String,
    pub customer_name: String,
    pub contact_person: Option<String>,
    pub contact_phone: Option<String>,
    pub status: i16,
    pub total_amount: rust_decimal::Decimal,
    pub remark: Option<String>,
    pub valid_until: Option<DateTime<Utc>>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub items: Vec<QuotationItem>,
}

impl<'r> FromRow<'r, PgRow> for Quotation {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Quotation {
            quotation_id: row.try_get("quotation_id")?,
            quotation_no: row.try_get("quotation_no")?,
            customer_name: row.try_get("customer_name")?,
            contact_person: row.try_get("contact_person")?,
            contact_phone: row.try_get("contact_phone")?,
            status: row.try_get("status")?,
            total_amount: row.try_get("total_amount")?,
            remark: row.try_get("remark")?,
            valid_until: row.try_get("valid_until")?,
            operator_id: row.try_get("operator_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            items: Vec::new(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct QuotationItem {
    pub item_id: i64,
    pub quotation_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: rust_decimal::Decimal,
    pub quantity: rust_decimal::Decimal,
    pub discount: rust_decimal::Decimal,
    pub subtotal: rust_decimal::Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, PgRow> for QuotationItem {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(QuotationItem {
            item_id: row.try_get("item_id")?,
            quotation_id: row.try_get("quotation_id")?,
            product_id: row.try_get("product_id")?,
            product_code: row.try_get("product_code")?,
            product_name: row.try_get("product_name")?,
            unit: row.try_get("unit")?,
            unit_price: row.try_get("unit_price")?,
            quantity: row.try_get("quantity")?,
            discount: row.try_get("discount")?,
            subtotal: row.try_get("subtotal")?,
            remark: row.try_get("remark")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct QuotationQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
