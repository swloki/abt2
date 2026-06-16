
use crate::shared::types::Result;

use super::model::TaxRate;

pub struct TaxRateRepo;

impl TaxRateRepo {
    /// 查询所有启用的税率
    pub async fn list_active(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<Vec<TaxRate>> {
        sqlx::query_as::<_, TaxRate>(
            r#"
            SELECT id, code, name, rate, tax_type, is_active,
                   created_at, updated_at, deleted_at
            FROM tax_rates
            WHERE is_active = TRUE AND deleted_at IS NULL
            ORDER BY rate
            "#,
        )
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// 按主键查询
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<TaxRate>> {
        sqlx::query_as::<_, TaxRate>(
            r#"
            SELECT id, code, name, rate, tax_type, is_active,
                   created_at, updated_at, deleted_at
            FROM tax_rates
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }

    /// 按编码查询
    pub async fn get_by_code(
        executor: &mut sqlx::postgres::PgConnection,
        code: &str,
    ) -> Result<Option<TaxRate>> {
        sqlx::query_as::<_, TaxRate>(
            r#"
            SELECT id, code, name, rate, tax_type, is_active,
                   created_at, updated_at, deleted_at
            FROM tax_rates
            WHERE code = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(code)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }

    /// 批量按 ID 查询（用于 PO 明细税率填充）
    pub async fn get_by_ids(
        executor: &mut sqlx::postgres::PgConnection,
        ids: &[i64],
    ) -> Result<Vec<TaxRate>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, TaxRate>(
            r#"
            SELECT id, code, name, rate, tax_type, is_active,
                   created_at, updated_at, deleted_at
            FROM tax_rates
            WHERE id = ANY($1) AND deleted_at IS NULL
            "#,
        )
        .bind(ids)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }
}
