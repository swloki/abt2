//! 科目映射数据库访问
use crate::shared::types::{PgExecutor, Result};

use super::model::AccountMapping;

pub struct GlMappingRepo;

impl GlMappingRepo {
    /// 查询产品级科目映射（mapping_key + product_id）
    pub async fn find_by_key_and_product(
        db: PgExecutor<'_>,
        mapping_key: &str,
        product_id: i64,
    ) -> Result<Option<AccountMapping>> {
        let row: Option<(i64, String, i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT id, mapping_key, account_id, product_id
            FROM gl_account_mappings
            WHERE mapping_key = $1 AND product_id = $2
            "#
        )
        .bind(mapping_key)
        .bind(product_id)
        .fetch_optional(db)
        .await?;

        Ok(row.map(|(id, mapping_key, account_id, product_id)| AccountMapping {
            id,
            mapping_key,
            account_id,
            product_id,
        }))
    }

    /// 查询全局默认科目映射（mapping_key + product_id IS NULL）
    pub async fn find_by_key_default(
        db: PgExecutor<'_>,
        mapping_key: &str,
    ) -> Result<Option<AccountMapping>> {
        let row: Option<(i64, String, i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT id, mapping_key, account_id, product_id
            FROM gl_account_mappings
            WHERE mapping_key = $1 AND product_id IS NULL
            "#
        )
        .bind(mapping_key)
        .fetch_optional(db)
        .await?;

        Ok(row.map(|(id, mapping_key, account_id, product_id)| AccountMapping {
            id,
            mapping_key,
            account_id,
            product_id,
        }))
    }
}
