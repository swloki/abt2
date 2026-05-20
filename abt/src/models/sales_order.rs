//! 销售订单数据模型
//!
//! 包含销售订单主表及行项目的实体定义和查询参数。

use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 销售订单主表
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SalesOrder {
    pub order_id: i64,
    pub order_no: String,
    pub quotation_id: Option<i64>,
    pub customer_name: String,
    pub contact_person: Option<String>,
    pub contact_phone: Option<String>,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub delivery_date: Option<NaiveDateTime>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
    pub items: Vec<SalesOrderItem>,
}

impl<'r> FromRow<'r, PgRow> for SalesOrder {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(SalesOrder {
            order_id: row.try_get("order_id")?,
            order_no: row.try_get("order_no")?,
            quotation_id: row.try_get("quotation_id").ok(),
            customer_name: row.try_get("customer_name")?,
            contact_person: row.try_get("contact_person").ok(),
            contact_phone: row.try_get("contact_phone").ok(),
            status: row.try_get("status")?,
            total_amount: row.try_get("total_amount")?,
            remark: row.try_get("remark").ok(),
            delivery_date: row.try_get("delivery_date").ok(),
            operator_id: row.try_get("operator_id").ok(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at").ok(),
            items: Vec::new(), // 通过独立查询填充
        })
    }
}

/// 销售订单行项目
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SalesOrderItem {
    pub item_id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub discount: Decimal,
    pub subtotal: Decimal,
    pub shipped_qty: Decimal,
    pub returned_qty: Decimal,
    pub remark: Option<String>,
    pub created_at: NaiveDateTime,
}

impl<'r> FromRow<'r, PgRow> for SalesOrderItem {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(SalesOrderItem {
            item_id: row.try_get("item_id")?,
            order_id: row.try_get("order_id")?,
            product_id: row.try_get("product_id")?,
            product_code: row.try_get("product_code").ok(),
            product_name: row.try_get("product_name").ok(),
            unit: row.try_get("unit").ok(),
            unit_price: row.try_get("unit_price")?,
            quantity: row.try_get("quantity")?,
            discount: row.try_get("discount")?,
            subtotal: row.try_get("subtotal")?,
            shipped_qty: row.try_get("shipped_qty")?,
            returned_qty: row.try_get("returned_qty")?,
            remark: row.try_get("remark").ok(),
            created_at: row.try_get("created_at")?,
        })
    }
}

/// 销售订单查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SalesOrderQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
