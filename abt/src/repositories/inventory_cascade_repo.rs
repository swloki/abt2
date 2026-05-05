//! 级联查询库存数据访问层

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::PgPool;

/// 第一次查询的扁平行结构（LEFT JOIN 产生，bom/child 列可能为 NULL）
#[derive(Debug, FromRow)]
pub struct CascadeNodeRow {
    pub root_product_id: i64,
    pub root_product_code: String,
    pub root_product_name: String,
    pub bom_id: Option<i64>,
    pub bom_name: Option<String>,
    pub node_id: Option<i64>,
    pub product_id: Option<i64>,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<Decimal>,
    pub loss_rate: Option<Decimal>,
    pub order: Option<i32>,
    pub parent_node_id: Option<i64>,
}

/// 库存汇总行
#[derive(Debug, FromRow)]
pub struct StockSummaryRow {
    pub product_id: i64,
    pub total_stock: Decimal,
}

pub struct InventoryCascadeRepo;

impl InventoryCascadeRepo {
    /// 查询产品的 BOM 引用及子节点结构
    ///
    /// 使用 CTE + LEFT JOIN：
    /// - 产品不存在 → 空结果
    /// - 产品存在但无 BOM 引用 → 1 行（bom 列为 NULL）
    /// - 有 BOM 引用 → 多行
    pub async fn find_cascade_nodes(
        pool: &PgPool,
        product_id: Option<i64>,
        product_code: Option<String>,
        max_results: i32,
    ) -> Result<Vec<CascadeNodeRow>> {
        let rows = sqlx::query_as::<_, CascadeNodeRow>(
            r#"
            WITH parent_product AS (
              SELECT product_id, product_code, pdt_name
              FROM products
              WHERE (product_id = $1 OR product_code = $2)
                AND deleted_at IS NULL
              LIMIT 1
            )
            SELECT
              pp.product_id AS root_product_id,
              pp.product_code AS root_product_code,
              pp.pdt_name AS root_product_name,
              b.bom_id,
              b.bom_name,
              child.id AS node_id,
              child.product_id,
              child.product_code,
              p_child.pdt_name AS product_name,
              child.unit,
              child.quantity,
              child.loss_rate,
              child."order",
              bn_parent.id AS parent_node_id
            FROM parent_product pp
            LEFT JOIN bom_nodes bn_parent ON bn_parent.product_id = pp.product_id
            LEFT JOIN bom b ON b.bom_id = bn_parent.bom_id
                      AND b.deleted_at IS NULL
            LEFT JOIN bom_nodes child ON child.parent_id = bn_parent.id
                       AND child.bom_id = bn_parent.bom_id
            LEFT JOIN products p_child ON p_child.product_id = child.product_id
                      AND p_child.deleted_at IS NULL
            ORDER BY b.bom_id, child."order"
            LIMIT $3
            "#,
        )
        .bind(product_id)
        .bind(product_code)
        .bind(max_results)
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
