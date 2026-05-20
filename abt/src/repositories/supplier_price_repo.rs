//! 供应商价格数据访问层
//!
//! 提供供应商价格的数据库 CRUD 操作。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{SupplierPrice, SupplierPriceDetail, SupplierPriceQuery};
use crate::repositories::Executor;

/// 供应商价格数据仓库
pub struct SupplierPriceRepo;

impl SupplierPriceRepo {
    /// 插入供应商价格，返回 price_id
    pub async fn insert(
        executor: Executor<'_>,
        supplier_id: i64,
        product_id: i64,
        unit_price: Decimal,
        valid_from: chrono::DateTime<chrono::Utc>,
        valid_until: chrono::DateTime<chrono::Utc>,
        operator_id: Option<i64>,
    ) -> Result<i64> {
        let price_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO supplier_prices (supplier_id, product_id, unit_price, valid_from, valid_until, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING price_id
            "#,
        )
        .bind(supplier_id)
        .bind(product_id)
        .bind(unit_price)
        .bind(valid_from)
        .bind(valid_until)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(price_id)
    }

    /// 查找供应商对某产品的当前有效价格
    pub async fn find_active(
        pool: &PgPool,
        supplier_id: i64,
        product_id: i64,
    ) -> Result<Option<SupplierPrice>> {
        let row = sqlx::query_as::<_, SupplierPrice>(
            r#"
            SELECT price_id, supplier_id, product_id, unit_price,
                   valid_from, valid_until, operator_id, created_at
            FROM supplier_prices
            WHERE supplier_id = $1
              AND product_id = $2
              AND NOW() BETWEEN valid_from AND valid_until
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(supplier_id)
        .bind(product_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 分页查询供应商价格列表（含产品信息）
    pub async fn query(
        pool: &PgPool,
        query: &SupplierPriceQuery,
    ) -> Result<Vec<SupplierPriceDetail>> {
        let mut qb = sqlx::QueryBuilder::new(
            r#"SELECT sp.price_id, sp.supplier_id, sp.product_id,
                      p.product_code, p.pdt_name as product_name, p.unit,
                      sp.unit_price, sp.valid_from, sp.valid_until,
                      sp.operator_id, sp.created_at
               FROM supplier_prices sp
               LEFT JOIN products p ON sp.product_id = p.product_id
               WHERE 1=1"#,
        );

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND sp.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(product_id) = query.product_id {
            qb.push(" AND sp.product_id = ");
            qb.push_bind(product_id);
        }

        if query.active_only == Some(true) {
            qb.push(" AND NOW() BETWEEN sp.valid_from AND sp.valid_until");
        }

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        qb.push(" ORDER BY sp.price_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb
            .build_query_as::<SupplierPriceDetail>()
            .fetch_all(pool)
            .await?;
        Ok(result)
    }

    /// 查询供应商价格总数
    pub async fn query_count(
        pool: &PgPool,
        query: &SupplierPriceQuery,
    ) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM supplier_prices sp WHERE 1=1",
        );

        if let Some(supplier_id) = query.supplier_id {
            qb.push(" AND sp.supplier_id = ");
            qb.push_bind(supplier_id);
        }

        if let Some(product_id) = query.product_id {
            qb.push(" AND sp.product_id = ");
            qb.push_bind(product_id);
        }

        if query.active_only == Some(true) {
            qb.push(" AND NOW() BETWEEN sp.valid_from AND sp.valid_until");
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }
}
