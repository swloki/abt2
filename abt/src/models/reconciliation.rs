use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReconciliationStatement {
    pub statement_id: i64,
    pub statement_no: String,
    pub customer_name: String,
    pub period_year: i16,
    pub period_month: i16,
    pub shipping_total: Decimal,
    pub return_total: Decimal,
    pub adjustment_total: Decimal,
    pub net_amount: Decimal,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub items: Vec<ReconciliationItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct ReconciliationItem {
    pub item_id: i64,
    pub statement_id: i64,
    pub source_type: String,
    pub source_id: Option<i64>,
    pub product_id: Option<i64>,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ReconciliationQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub period_year: Option<i16>,
    pub period_month: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl<'r> FromRow<'r, PgRow> for ReconciliationStatement {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(ReconciliationStatement {
            statement_id: row.try_get("statement_id")?,
            statement_no: row.try_get("statement_no")?,
            customer_name: row.try_get("customer_name")?,
            period_year: row.try_get("period_year")?,
            period_month: row.try_get("period_month")?,
            shipping_total: row.try_get("shipping_total")?,
            return_total: row.try_get("return_total")?,
            adjustment_total: row.try_get("adjustment_total")?,
            net_amount: row.try_get("net_amount")?,
            status: row.try_get("status")?,
            remark: row.try_get("remark")?,
            operator_id: row.try_get("operator_id")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at")?,
            items: Vec::new(),
        })
    }
}
