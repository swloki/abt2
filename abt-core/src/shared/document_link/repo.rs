use sqlx::{FromRow, Row};

use super::model::{DocumentLink, LinkRequest};
use crate::shared::enums::DocumentType;

pub struct DocumentLinkRepo;

impl DocumentLinkRepo {
    /// 在事务中批量 INSERT 单据关联
    /// path = source_prefix.source_id（顶层）或 parent_path + .target_prefix.target_id
    pub async fn batch_insert(
        executor: &mut sqlx::postgres::PgConnection,
        requests: &[LinkRequest],
        created_by: Option<i64>,
    ) -> Result<Vec<DocumentLink>, sqlx::Error> {
        let mut results = Vec::with_capacity(requests.len());

        for req in requests {
            let path = format!(
                "{}.{}.{}.{}",
                req.source_type.prefix(),
                req.source_id,
                req.target_type.prefix(),
                req.target_id
            );
            let depth = 1i32;

            let row = sqlx::query(
                r#"
                INSERT INTO document_links
                    (source_type, source_id, target_type, target_id, link_type, path, depth, created_by)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING id, source_type, source_id, target_type, target_id, link_type, path, depth, created_at, created_by
                "#,
            )
            .bind(req.source_type)
            .bind(req.source_id)
            .bind(req.target_type)
            .bind(req.target_id)
            .bind(req.link_type)
            .bind(&path)
            .bind(depth)
            .bind(created_by)
            .fetch_one(&mut *executor)
            .await?;

            results.push(DocumentLink::from_row(&row)?);
        }

        Ok(results)
    }

    /// 双向分页查询：同时搜索 source→target 和 target→source 方向
    pub async fn find_linked(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: DocumentType,
        source_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<DocumentLink>, u64), sqlx::Error> {
        // Count
        let count_sql = r#"
            SELECT COUNT(*) AS cnt FROM document_links
            WHERE (source_type = $1 AND source_id = $2)
               OR (target_type = $1 AND target_id = $2)
        "#;
        let count_row = sqlx::query(count_sql)
            .bind(source_type)
            .bind(source_id)
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let data_sql = r#"
            SELECT id, source_type, source_id, target_type, target_id, link_type, path, depth, created_at, created_by
            FROM document_links
            WHERE (source_type = $1 AND source_id = $2)
               OR (target_type = $1 AND target_id = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
        "#;
        let rows = sqlx::query(data_sql)
            .bind(source_type)
            .bind(source_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *executor)
            .await?;

        let items: Vec<DocumentLink> = rows
            .iter()
            .filter_map(|row| DocumentLink::from_row(row).ok())
            .collect();

        Ok((items, total as u64))
    }
}
