use rust_decimal::Decimal;

use crate::shared::types::Result;

use super::model::SupplierProductPrice;

pub struct SupplierProductPriceRepo;

impl SupplierProductPriceRepo {
    /// 按供应商+产品匹配最优价格（按优先级+有效期+起订量）
    pub async fn match_best_price(
        executor: &mut sqlx::postgres::PgConnection,
        supplier_id: i64,
        product_id: i64,
        quantity: Decimal,
    ) -> Result<Option<SupplierProductPrice>> {
        let today = chrono::Local::now().date_naive();
        sqlx::query_as::<_, SupplierProductPrice>(
            r#"
            SELECT id, supplier_id, product_id, supplier_item_code, supplier_item_name,
                   min_order_qty, price, currency_code, discount_pct, lead_time_days,
                   tax_rate_id, valid_from, valid_until, sequence, is_active,
                   created_at, updated_at, deleted_at
            FROM supplier_product_prices
            WHERE supplier_id = $1
              AND product_id = $2
              AND is_active = TRUE
              AND deleted_at IS NULL
              AND min_order_qty <= $3
              AND (valid_from IS NULL OR valid_from <= $4)
              AND (valid_until IS NULL OR valid_until >= $4)
            ORDER BY sequence, price
            LIMIT 1
            "#,
        )
        .bind(supplier_id)
        .bind(product_id)
        .bind(quantity)
        .bind(today)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }

    /// 获取上次采购价
    pub async fn get_last_purchase_price(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
    ) -> Result<Option<(Decimal, chrono::NaiveDate)>> {
        let row = sqlx::query(
            r#"
            SELECT poi.unit_price, po.order_date
            FROM purchase_order_items poi
            JOIN purchase_orders po ON po.id = poi.order_id
            WHERE poi.product_id = $1
              AND po.status IN (2, 3, 4)
              AND po.deleted_at IS NULL
            ORDER BY po.order_date DESC
            LIMIT 1
            "#,
        )
        .bind(product_id)
        .fetch_optional(&mut *executor)
        .await?;

        match row {
            Some(r) => {
                use sqlx::Row;
                Ok(Some((
                    r.try_get("unit_price")?,
                    r.try_get("order_date")?,
                )))
            }
            None => Ok(None),
        }
    }

    /// 按供应商查询
    pub async fn list_by_supplier(
        executor: &mut sqlx::postgres::PgConnection,
        supplier_id: i64,
    ) -> Result<Vec<SupplierProductPrice>> {
        sqlx::query_as::<_, SupplierProductPrice>(
            r#"
            SELECT id, supplier_id, product_id, supplier_item_code, supplier_item_name,
                   min_order_qty, price, currency_code, discount_pct, lead_time_days,
                   tax_rate_id, valid_from, valid_until, sequence, is_active,
                   created_at, updated_at, deleted_at
            FROM supplier_product_prices
            WHERE supplier_id = $1 AND deleted_at IS NULL
            ORDER BY product_id, sequence
            "#,
        )
        .bind(supplier_id)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// 按产品查询
    pub async fn list_by_product(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
    ) -> Result<Vec<SupplierProductPrice>> {
        sqlx::query_as::<_, SupplierProductPrice>(
            r#"
            SELECT id, supplier_id, product_id, supplier_item_code, supplier_item_name,
                   min_order_qty, price, currency_code, discount_pct, lead_time_days,
                   tax_rate_id, valid_from, valid_until, sequence, is_active,
                   created_at, updated_at, deleted_at
            FROM supplier_product_prices
            WHERE product_id = $1 AND is_active = TRUE AND deleted_at IS NULL
            ORDER BY sequence, price
            "#,
        )
        .bind(product_id)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// 插入供应商产品价格记录
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        supplier_id: i64,
        product_id: i64,
        unit_price: Decimal,
        currency_code: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO supplier_product_prices
                (supplier_id, product_id, price, currency_code, min_order_qty)
            VALUES ($1, $2, $3, $4, 1)
            "#,
        )
        .bind(supplier_id)
        .bind(product_id)
        .bind(unit_price)
        .bind(currency_code)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 按主键删除
    pub async fn delete_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<()> {
        sqlx::query("UPDATE supplier_product_prices SET deleted_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }
}
