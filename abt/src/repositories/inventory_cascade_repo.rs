//! 级联查询库存数据访问层

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::PgPool;

/// 产品基本信息
#[derive(Debug, FromRow)]
pub struct ProductInfoRow {
    pub product_id: i64,
    pub product_code: String,
    pub pdt_name: String,
}

/// 产品在已发布 BOM 中的引用（入口节点）
#[derive(Debug, FromRow)]
pub struct BomRefRow {
    pub bom_id: i64,
    pub bom_name: String,
    pub entry_node_id: i64,
}

/// BOM 节点扁平行
#[derive(Debug, FromRow)]
pub struct CascadeNodeFlat {
    pub id: i64,
    pub bom_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub quantity: Decimal,
    pub parent_id: Option<i64>,
    pub loss_rate: Decimal,
    pub order: i32,
    pub unit: Option<String>,
}

/// 库存汇总行
#[derive(Debug, FromRow)]
pub struct StockSummaryRow {
    pub product_id: i64,
    pub total_stock: Decimal,
}

pub struct InventoryCascadeRepo;

impl InventoryCascadeRepo {
    pub async fn find_product(
        pool: &PgPool,
        product_id: Option<i64>,
        product_code: Option<String>,
    ) -> Result<Option<ProductInfoRow>> {
        let row = if let Some(id) = product_id {
            sqlx::query_as::<_, ProductInfoRow>(
                "SELECT product_id, product_code, pdt_name FROM products WHERE product_id = $1 LIMIT 1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await?
        } else if let Some(code) = product_code {
            sqlx::query_as::<_, ProductInfoRow>(
                "SELECT product_id, product_code, pdt_name FROM products WHERE product_code = $1 LIMIT 1",
            )
            .bind(code)
            .fetch_optional(pool)
            .await?
        } else {
            return Ok(None);
        };
        Ok(row)
    }

    /// 批量查询产品基本信息（不加载 JSONB meta）
    pub async fn find_products_by_ids(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<Vec<ProductInfoRow>> {
        if product_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query_as::<_, ProductInfoRow>(
            "SELECT product_id, product_code, pdt_name FROM products WHERE product_id = ANY($1)",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 查找产品被引用的最新 N 个已发布 BOM（每个 BOM 只取一个入口节点）
    pub async fn find_published_bom_refs(
        pool: &PgPool,
        product_id: i64,
        limit: i32,
    ) -> Result<Vec<BomRefRow>> {
        let rows = sqlx::query_as::<_, BomRefRow>(
            r#"
            SELECT DISTINCT ON (bn.bom_id) bn.bom_id, b.bom_name, bn.id AS entry_node_id
            FROM bom_nodes bn
            JOIN bom b ON b.bom_id = bn.bom_id AND b.status = 'published'
            WHERE bn.product_id = $1
            ORDER BY bn.bom_id DESC, bn.id ASC
            LIMIT $2
            "#,
        )
        .bind(product_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 批量加载指定 BOM 的全部节点
    pub async fn find_nodes_by_bom_ids(
        pool: &PgPool,
        bom_ids: &[i64],
    ) -> Result<Vec<CascadeNodeFlat>> {
        if bom_ids.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query_as::<_, CascadeNodeFlat>(
            r#"
            SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit
            FROM bom_nodes
            WHERE bom_id = ANY($1)
            ORDER BY "order"
            "#,
        )
        .bind(bom_ids)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 批量查询产品库存汇总
    pub async fn find_stock_summary(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<Vec<StockSummaryRow>> {
        if product_ids.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query_as::<_, StockSummaryRow>(
            r#"
            SELECT
              i.product_id,
              SUM(i.quantity) AS total_stock
            FROM inventory i
            WHERE i.product_id = ANY($1)
            GROUP BY i.product_id
            "#,
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
