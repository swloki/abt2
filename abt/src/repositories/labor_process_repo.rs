//! 劳务工序数据访问层
//!
//! 提供扁平 bom_labor_process 表的 CRUD 和批量操作。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::Executor;

/// 无人工成本的 BOM 信息
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomWithoutLaborCost {
    pub bom_id: i64,
    pub bom_name: String,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 劳务工序仓库
pub struct LaborProcessRepo;

impl LaborProcessRepo {
    // ========================================================================
    // 查询
    // ========================================================================

    /// 按产品查询工序（支持按名称模糊搜索）
    pub async fn find_by_product_code(
        pool: &PgPool,
        product_code: &str,
        keyword: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<BomLaborProcess>> {
        let offset = (page.max(1) - 1) * page_size.clamp(1, 100);
        let items: Vec<BomLaborProcess> = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_as(
                "SELECT id, product_code, process_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at \
                 FROM bom_labor_process \
                 WHERE product_code = $1 AND name ILIKE $2 \
                 ORDER BY sort_order ASC, id ASC \
                 LIMIT $3 OFFSET $4"
            )
            .bind(product_code)
            .bind(&pattern)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, product_code, process_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at \
                 FROM bom_labor_process \
                 WHERE product_code = $1 \
                 ORDER BY sort_order ASC, id ASC \
                 LIMIT $2 OFFSET $3"
            )
            .bind(product_code)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    /// 按产品统计工序数量（支持按名称模糊搜索）
    pub async fn count_by_product_code(
        pool: &PgPool,
        product_code: &str,
        keyword: Option<&str>,
    ) -> Result<i64> {
        let count: i64 = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM bom_labor_process WHERE product_code = $1 AND name ILIKE $2"
            )
            .bind(product_code)
            .bind(&pattern)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM bom_labor_process WHERE product_code = $1"
            )
            .bind(product_code)
            .fetch_one(pool)
            .await?
        };
        Ok(count)
    }

    // ========================================================================
    // 写入
    // ========================================================================

    /// 创建工序
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        executor: Executor<'_>,
        product_code: &str,
        process_code: Option<&str>,
        name: &str,
        unit_price: Decimal,
        quantity: Decimal,
        sort_order: i32,
        remark: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO bom_labor_process (product_code, process_code, name, unit_price, quantity, sort_order, remark)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#
        )
        .bind(product_code)
        .bind(process_code)
        .bind(name)
        .bind(unit_price)
        .bind(quantity)
        .bind(sort_order)
        .bind(remark)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 更新工序
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        executor: Executor<'_>,
        id: i64,
        product_code: &str,
        process_code: Option<&str>,
        name: &str,
        unit_price: Decimal,
        quantity: Decimal,
        sort_order: i32,
        remark: Option<&str>,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE bom_labor_process
            SET product_code = $1, process_code = $2, name = $3, unit_price = $4, quantity = $5, sort_order = $6, remark = $7, updated_at = NOW()
            WHERE id = $8
            "#
        )
        .bind(product_code)
        .bind(process_code)
        .bind(name)
        .bind(unit_price)
        .bind(quantity)
        .bind(sort_order)
        .bind(remark)
        .bind(id)
        .execute(executor)
        .await?;
        if result.rows_affected() == 0 {
            return Err(common::error::ServiceError::NotFound {
                resource: "劳务工序".to_string(),
                id: id.to_string(),
            }.into());
        }
        Ok(())
    }

    /// 删除工序（验证 product_code 匹配）
    pub async fn delete(executor: Executor<'_>, id: i64, product_code: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM bom_labor_process WHERE id = $1 AND product_code = $2"
        )
        .bind(id)
        .bind(product_code)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    // ========================================================================
    // Excel 批量操作
    // ========================================================================

    /// 删除产品的所有工序（用于导入前清除）
    pub async fn delete_by_product_code(
        executor: Executor<'_>,
        product_code: &str,
    ) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM bom_labor_process WHERE product_code = $1"
        )
        .bind(product_code)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 批量插入工序（用于导入）
    pub async fn batch_insert(
        executor: Executor<'_>,
        product_code: &str,
        items: &[crate::models::ValidLaborProcessRow],
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut builder: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            "INSERT INTO bom_labor_process (product_code, process_code, name, unit_price, quantity, sort_order, remark) "
        );
        builder.push_values(items.iter(), |mut b, row| {
            b.push_bind(product_code);
            b.push_bind(&row.process_code);
            b.push_bind(&row.name);
            b.push_bind(row.unit_price);
            b.push_bind(row.quantity);
            b.push_bind(row.sort_order);
            b.push_bind(&row.remark);
        });
        builder.build().execute(executor).await?;
        Ok(())
    }

    /// 查询产品的所有工序（用于导出，按 sort_order 排序）
    pub async fn list_all_by_product_code(
        pool: &PgPool,
        product_code: &str,
    ) -> Result<Vec<BomLaborProcess>> {
        let items = sqlx::query_as(
            "SELECT id, product_code, process_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at \
             FROM bom_labor_process \
             WHERE product_code = $1 \
             ORDER BY sort_order ASC, id ASC"
        )
        .bind(product_code)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// 查询没有录入人工成本的 BOM 列表
    pub async fn find_boms_without_labor_cost(pool: &PgPool) -> Result<Vec<BomWithoutLaborCost>> {
        let items = sqlx::query_as::<_, BomWithoutLaborCost>(
            r#"
            SELECT b.bom_id, b.bom_name,
                   p.meta->>'product_code' AS product_code,
                   p.pdt_name AS product_name,
                   b.create_at AS created_at
            FROM bom b
            JOIN bom_nodes bn ON bn.bom_id = b.bom_id AND bn.parent_id IS NULL
            JOIN products p ON bn.product_id = p.product_id
            WHERE NOT EXISTS (
                  SELECT 1 FROM bom_labor_process blp
                  WHERE blp.product_code = p.meta->>'product_code'
              )
            ORDER BY b.bom_id DESC
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(items)
    }
}
