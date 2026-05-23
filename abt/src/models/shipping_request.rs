use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ShippingRequest {
    pub request_id: i64,
    pub request_no: String,
    pub order_id: i64,
    pub customer_name: String,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub items: Vec<ShippingRequestItem>,
}

impl<'r> FromRow<'r, PgRow> for ShippingRequest {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(ShippingRequest {
            request_id: row.try_get("request_id")?,
            request_no: row.try_get("request_no")?,
            order_id: row.try_get("order_id")?,
            customer_name: row.try_get("customer_name")?,
            status: row.try_get("status")?,
            remark: row.try_get("remark")?,
            operator_id: row.try_get("operator_id")?,
            confirmed_at: row.try_get("confirmed_at")?,
            shipped_at: row.try_get("shipped_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            items: Vec::new(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ShippingRequestItem {
    pub item_id: i64,
    pub request_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub quantity: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, PgRow> for ShippingRequestItem {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(ShippingRequestItem {
            item_id: row.try_get("item_id")?,
            request_id: row.try_get("request_id")?,
            order_item_id: row.try_get("order_item_id")?,
            product_id: row.try_get("product_id")?,
            product_code: row.try_get("product_code")?,
            product_name: row.try_get("product_name")?,
            unit: row.try_get("unit")?,
            quantity: row.try_get("quantity")?,
            remark: row.try_get("remark")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ShippingRequestQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub order_id: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
