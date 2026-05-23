use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SalesReturn {
    pub return_id: i64,
    pub return_no: String,
    pub request_id: i64,
    pub order_id: i64,
    pub customer_name: String,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub reason: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub items: Vec<SalesReturnItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SalesReturnItem {
    pub item_id: i64,
    pub return_id: i64,
    pub request_item_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SalesReturnQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub order_id: Option<i64>,
    pub request_id: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl<'r> FromRow<'r, PgRow> for SalesReturn {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(SalesReturn {
            return_id: row.try_get("return_id")?,
            return_no: row.try_get("return_no")?,
            request_id: row.try_get("request_id")?,
            order_id: row.try_get("order_id")?,
            customer_name: row.try_get("customer_name")?,
            status: row.try_get("status")?,
            total_amount: row.try_get("total_amount")?,
            remark: row.try_get("remark")?,
            reason: row.try_get("reason")?,
            operator_id: row.try_get("operator_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at")?,
            items: Vec::new(),
        })
    }
}
