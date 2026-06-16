use rust_decimal::Decimal;

use crate::shared::types::{PageParams, Result};

use super::model::{PriceListQuery, PriceUpsertRequest, PriceView, SupplierProductPrice};

/// JOIN 供应商名 + 产品名的视图列
const PRICE_VIEW_COLUMNS: &str = "sp.id, sp.supplier_id, sp.product_id, sp.supplier_item_code, sp.supplier_item_name,
       sp.min_order_qty, sp.price, sp.currency_code, sp.discount_pct, sp.lead_time_days,
       sp.tax_rate_id, sp.valid_from, sp.valid_until, sp.sequence, sp.is_active,
       sp.created_at, sp.updated_at,
       s.supplier_name, s.supplier_code, p.product_code, p.pdt_name AS product_name";

const PRICE_JOIN: &str = "FROM supplier_product_prices sp
       LEFT JOIN suppliers s ON s.supplier_id = sp.supplier_id
       LEFT JOIN products p ON p.product_id = sp.product_id";

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
                Ok(Some((r.try_get("unit_price")?, r.try_get("order_date")?)))
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

    /// 价格目录列表（JOIN 名称，支持筛选 + 关键词 + 分页）
    pub async fn list_prices(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &PriceListQuery,
        page: PageParams,
    ) -> Result<(Vec<PriceView>, u64)> {
        let (where_sql, next_idx) = Self::build_price_where(filter);

        // count
        let count_sql = format!("SELECT COUNT(*) {PRICE_JOIN} WHERE {where_sql}");
        let mut count_q =
            sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = filter.supplier_id {
            count_q = count_q.bind(v);
        }
        if let Some(v) = filter.product_id {
            count_q = count_q.bind(v);
        }
        if let Some(ref v) = filter.currency_code {
            count_q = count_q.bind(v);
        }
        if let Some(v) = filter.is_active {
            count_q = count_q.bind(v);
        }
        if let Some(ref v) = filter.keyword {
            count_q = count_q.bind(format!("%{v}%"));
        }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // list（与 count 绑定顺序一致）
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let list_sql = format!(
            "SELECT {PRICE_VIEW_COLUMNS} {PRICE_JOIN} WHERE {where_sql} \
             ORDER BY sp.sequence, sp.id LIMIT ${next_idx} OFFSET ${}",
            next_idx + 1
        );
        let mut list_q = sqlx::query_as::<_, PriceView>(sqlx::AssertSqlSafe(list_sql));
        if let Some(v) = filter.supplier_id {
            list_q = list_q.bind(v);
        }
        if let Some(v) = filter.product_id {
            list_q = list_q.bind(v);
        }
        if let Some(ref v) = filter.currency_code {
            list_q = list_q.bind(v);
        }
        if let Some(v) = filter.is_active {
            list_q = list_q.bind(v);
        }
        if let Some(ref v) = filter.keyword {
            list_q = list_q.bind(format!("%{v}%"));
        }
        let items = list_q.bind(limit).bind(offset).fetch_all(&mut *executor).await?;
        Ok((items, total))
    }

    /// 单条价格视图（编辑回填）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<PriceView>> {
        let sql =
            format!("SELECT {PRICE_VIEW_COLUMNS} {PRICE_JOIN} WHERE sp.id = $1 AND sp.deleted_at IS NULL");
        sqlx::query_as::<_, PriceView>(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// 插入完整字段的价格记录
    pub async fn insert_full(
        executor: &mut sqlx::postgres::PgConnection,
        req: &PriceUpsertRequest,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO supplier_product_prices
                (supplier_id, product_id, price, currency_code, min_order_qty, discount_pct,
                 lead_time_days, tax_rate_id, valid_from, valid_until, sequence,
                 supplier_item_code, supplier_item_name, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id
            "#,
        )
        .bind(req.supplier_id)
        .bind(req.product_id)
        .bind(req.price)
        .bind(&req.currency_code)
        .bind(req.min_order_qty)
        .bind(req.discount_pct)
        .bind(req.lead_time_days)
        .bind(req.tax_rate_id)
        .bind(req.valid_from)
        .bind(req.valid_until)
        .bind(req.sequence)
        .bind(&req.supplier_item_code)
        .bind(&req.supplier_item_name)
        .bind(req.is_active)
        .fetch_one(&mut *executor)
        .await?;
        Ok(id)
    }

    /// 插入最小字段价格记录（PO confirm 自动建价等内部场景）
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

    /// 更新价格记录（全字段）
    pub async fn update_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        req: &PriceUpsertRequest,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE supplier_product_prices SET
                supplier_id = $1, product_id = $2, price = $3, currency_code = $4,
                min_order_qty = $5, discount_pct = $6, lead_time_days = $7, tax_rate_id = $8,
                valid_from = $9, valid_until = $10, sequence = $11,
                supplier_item_code = $12, supplier_item_name = $13, is_active = $14,
                updated_at = NOW()
            WHERE id = $15 AND deleted_at IS NULL
            "#,
        )
        .bind(req.supplier_id)
        .bind(req.product_id)
        .bind(req.price)
        .bind(&req.currency_code)
        .bind(req.min_order_qty)
        .bind(req.discount_pct)
        .bind(req.lead_time_days)
        .bind(req.tax_rate_id)
        .bind(req.valid_from)
        .bind(req.valid_until)
        .bind(req.sequence)
        .bind(&req.supplier_item_code)
        .bind(&req.supplier_item_name)
        .bind(req.is_active)
        .bind(id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 按主键软删除
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

    // ---- 动态 WHERE 构造（筛选 + 关键词）----

    fn build_price_where(filter: &PriceListQuery) -> (String, usize) {
        let mut clauses = vec!["sp.deleted_at IS NULL".to_string()];
        let mut idx = 1usize;
        if filter.supplier_id.is_some() {
            clauses.push(format!("sp.supplier_id = ${idx}"));
            idx += 1;
        }
        if filter.product_id.is_some() {
            clauses.push(format!("sp.product_id = ${idx}"));
            idx += 1;
        }
        if filter.currency_code.is_some() {
            clauses.push(format!("sp.currency_code = ${idx}"));
            idx += 1;
        }
        if filter.is_active.is_some() {
            clauses.push(format!("sp.is_active = ${idx}"));
            idx += 1;
        }
        if filter.keyword.is_some() {
            clauses.push(format!(
                "(s.supplier_name ILIKE ${idx} OR p.pdt_name ILIKE ${idx} \
                 OR p.product_code ILIKE ${idx} OR sp.supplier_item_code ILIKE ${idx} \
                 OR sp.supplier_item_name ILIKE ${idx})"
            ));
            idx += 1;
        }
        (clauses.join(" AND "), idx)
    }
}
