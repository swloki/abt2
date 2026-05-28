use rust_decimal::Decimal;
use sqlx::FromRow;

use crate::shared::types::{PgExecutor, Result};

#[derive(Debug, FromRow)]
pub struct ProductInfoRow {
    pub product_id: i64,
    pub product_code: String,
    pub pdt_name: String,
}

#[derive(Debug, FromRow)]
pub struct BomRefRow {
    pub bom_id: i64,
    pub bom_name: String,
    pub entry_node_id: i64,
}

#[derive(Debug, FromRow)]
pub struct CascadeNodeFlat {
    pub node_id: i64,
    pub bom_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub quantity: Decimal,
    pub parent_id: Option<i64>,
    pub loss_rate: Decimal,
    pub order_num: i32,
    pub unit: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct StockSummaryRow {
    pub product_id: i64,
    pub total_stock: Decimal,
}

pub struct InventoryCascadeRepo;

impl InventoryCascadeRepo {
    pub async fn find_product_by_id(
        db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Option<ProductInfoRow>> {
        let row = sqlx::query_as::<_, ProductInfoRow>(
            "SELECT product_id, product_code, pdt_name FROM products WHERE product_id = $1 LIMIT 1",
        )
        .bind(product_id)
        .fetch_optional(db)
        .await?;
        Ok(row)
    }

    pub async fn find_product_by_code(
        db: PgExecutor<'_>,
        code: &str,
    ) -> Result<Option<ProductInfoRow>> {
        let row = sqlx::query_as::<_, ProductInfoRow>(
            "SELECT product_id, product_code, pdt_name FROM products WHERE product_code = $1 LIMIT 1",
        )
        .bind(code)
        .fetch_optional(db)
        .await?;
        Ok(row)
    }

    pub async fn find_bom_refs(
        db: PgExecutor<'_>,
        product_id: i64,
        limit: i32,
    ) -> Result<Vec<BomRefRow>> {
        let rows = sqlx::query_as::<_, BomRefRow>(
            r#"SELECT DISTINCT ON (bn.bom_id) bn.bom_id, b.bom_name, bn.node_id AS entry_node_id
               FROM bom_nodes bn
               JOIN boms b ON b.bom_id = bn.bom_id AND b.deleted_at IS NULL
               WHERE bn.product_id = $1
               ORDER BY bn.bom_id DESC, bn.node_id ASC
               LIMIT $2"#,
        )
        .bind(product_id)
        .bind(limit)
        .fetch_all(db)
        .await?;
        Ok(rows)
    }

    pub async fn find_bom_nodes(
        db: PgExecutor<'_>,
        bom_ids: &[i64],
    ) -> Result<Vec<CascadeNodeFlat>> {
        let rows = sqlx::query_as::<_, CascadeNodeFlat>(
            r#"SELECT node_id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, order_num, unit
               FROM bom_nodes
               WHERE bom_id = ANY($1)
               ORDER BY order_num"#,
        )
        .bind(bom_ids)
        .fetch_all(db)
        .await?;
        Ok(rows)
    }

    pub async fn find_products_by_ids(
        db: PgExecutor<'_>,
        ids: &[i64],
    ) -> Result<Vec<ProductInfoRow>> {
        let rows = sqlx::query_as::<_, ProductInfoRow>(
            "SELECT product_id, product_code, pdt_name FROM products WHERE product_id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(db)
        .await?;
        Ok(rows)
    }

    pub async fn query_stock_summary(
        db: PgExecutor<'_>,
        product_ids: &[i64],
    ) -> Result<Vec<StockSummaryRow>> {
        let rows = sqlx::query_as::<_, StockSummaryRow>(
            "SELECT product_id, SUM(quantity) AS total_stock FROM stock_ledger WHERE product_id = ANY($1) GROUP BY product_id",
        )
        .bind(product_ids)
        .fetch_all(db)
        .await?;
        Ok(rows)
    }
}
