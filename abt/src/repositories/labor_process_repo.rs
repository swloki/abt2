//! BOM 人工工序数据访问层

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{BomLaborProcess, CreateLaborProcessRequest, UpdateLaborProcessRequest};
use crate::repositories::Executor;

/// BOM 人工工序仓库
pub struct LaborProcessRepo;

impl LaborProcessRepo {
    /// 插入人工工序
    pub async fn insert(
        executor: Executor<'_>,
        req: &CreateLaborProcessRequest,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO bom_labor_process
                (product_code, name, unit_price, quantity, sort_order, remark)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            &req.product_code,
            &req.name,
            req.unit_price,
            req.quantity,
            req.sort_order,
            req.remark.as_deref().unwrap_or("")
        )
        .fetch_one(executor)
        .await?;

        Ok(id)
    }

    /// 更新人工工序
    pub async fn update(
        executor: Executor<'_>,
        req: &UpdateLaborProcessRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE bom_labor_process
            SET name = $1, unit_price = $2, quantity = $3, sort_order = $4, remark = $5, updated_at = NOW()
            WHERE id = $6 AND product_code = $7
            "#,
            &req.name,
            req.unit_price,
            req.quantity,
            req.sort_order,
            req.remark.as_deref().unwrap_or(""),
            req.id,
            &req.product_code
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 删除人工工序
    pub async fn delete(executor: Executor<'_>, id: i64, product_code: &str) -> Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM bom_labor_process WHERE id = $1 AND product_code = $2",
            id,
            product_code
        )
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 根据产品编码查找工序列表
    pub async fn find_by_product_code(
        pool: &PgPool,
        product_code: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<BomLaborProcess>> {
        let offset = (page.max(1) - 1) * page_size.clamp(1, 100);

        let items = sqlx::query_as!(
            BomLaborProcess,
            r#"
            SELECT id, product_code, name, unit_price, quantity,
                   sort_order, remark, created_at, updated_at
            FROM bom_labor_process
            WHERE product_code = $1
            ORDER BY sort_order ASC, id ASC
            LIMIT $2 OFFSET $3
            "#,
            product_code,
            page_size as i32,
            offset as i32
        )
        .fetch_all(pool)
        .await?;

        Ok(items)
    }

    /// 根据产品编码统计工序数量
    pub async fn count_by_product_code(pool: &PgPool, product_code: &str) -> Result<i64> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM bom_labor_process WHERE product_code = $1",
        )
        .bind(product_code)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    /// 根据产品编码删除所有工序（用于导入覆盖）
    pub async fn delete_by_product_code(executor: Executor<'_>, product_code: &str) -> Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM bom_labor_process WHERE product_code = $1",
            product_code
        )
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 批量插入工序（用于导入）
    pub async fn batch_insert(
        executor: Executor<'_>,
        product_code: &str,
        items: &[(String, Decimal, Decimal, i32, Option<String>)],
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // Build batch insert with parameterized queries
        let mut query_builder: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            "INSERT INTO bom_labor_process (product_code, name, unit_price, quantity, sort_order, remark) ",
        );

        query_builder.push_values(items.iter(), |mut b, item| {
            let (name, unit_price, quantity, sort_order, remark) = item;
            b.push_bind(product_code);
            b.push_bind(name);
            b.push_bind(unit_price);
            b.push_bind(quantity);
            b.push_bind(*sort_order);
            b.push_bind(remark);
        });

        query_builder.build().execute(executor).await?;

        Ok(())
    }
}
