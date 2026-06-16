use crate::shared::types::Result;

use super::model::{DocumentLink, LinkRequest};
use crate::shared::enums::{DocumentType, LinkType};

pub struct DocumentLinkRepo;

impl DocumentLinkRepo {
    /// 在事务中批量 INSERT 单据关联
    /// path = source_prefix.source_id（顶层）或 parent_path + .target_prefix.target_id
    pub async fn batch_insert(
        executor: &mut sqlx::postgres::PgConnection,
        requests: &[LinkRequest],
        created_by: Option<i64>,
    ) -> Result<Vec<DocumentLink>> {
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

            let row = sqlx::query!(
                r#"
                INSERT INTO document_links
                    (source_type, source_id, target_type, target_id, link_type, path, depth, created_by)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING id, source_type as "source_type: i16", source_id, target_type as "target_type: i16", target_id, link_type as "link_type: i16", path, depth, created_at, created_by
                "#,
                req.source_type.as_i16(),
                req.source_id,
                req.target_type.as_i16(),
                req.target_id,
                req.link_type.as_i16(),
                &path,
                depth,
                created_by,
            )
            .fetch_one(&mut *executor)
            .await?;

            results.push(DocumentLink {
                id: row.id,
                source_type: DocumentType::from_i16(row.source_type).unwrap(),
                source_id: row.source_id,
                target_type: DocumentType::from_i16(row.target_type).unwrap(),
                target_id: row.target_id,
                link_type: LinkType::from_i16(row.link_type).unwrap(),
                path: row.path,
                depth: row.depth,
                created_at: row.created_at,
                created_by: row.created_by,
            });
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
    ) -> Result<(Vec<DocumentLink>, u64)> {
        // Count
        let total: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM document_links
            WHERE (source_type = $1 AND source_id = $2)
               OR (target_type = $1 AND target_id = $2)
            "#,
            source_type.as_i16(),
            source_id,
        )
        .fetch_one(&mut *executor)
        .await?
        .unwrap_or(0);

        // Data
        let rows = sqlx::query!(
            r#"
            SELECT id, source_type as "source_type: i16", source_id, target_type as "target_type: i16", target_id, link_type as "link_type: i16", path, depth, created_at, created_by
            FROM document_links
            WHERE (source_type = $1 AND source_id = $2)
               OR (target_type = $1 AND target_id = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            source_type.as_i16(),
            source_id,
            limit,
            offset,
        )
        .fetch_all(&mut *executor)
        .await?;

        let items: Vec<DocumentLink> = rows
            .into_iter()
            .map(|r| DocumentLink {
                id: r.id,
                source_type: DocumentType::from_i16(r.source_type).unwrap(),
                source_id: r.source_id,
                target_type: DocumentType::from_i16(r.target_type).unwrap(),
                target_id: r.target_id,
                link_type: LinkType::from_i16(r.link_type).unwrap(),
                path: r.path,
                depth: r.depth,
                created_at: r.created_at,
                created_by: r.created_by,
            })
            .collect();

        Ok((items, total as u64))
    }

    /// 双向按类型查找关联单据 ID：返回与 (anchor_type, anchor_id) 关联的 other_type 单据 ID
    /// 同时覆盖 source→target 与 target→source 两个方向（如：查某工单关联的所有领料单）
    pub async fn find_linked_ids_by_type(
        executor: &mut sqlx::postgres::PgConnection,
        anchor_type: DocumentType,
        anchor_id: i64,
        other_type: DocumentType,
    ) -> Result<Vec<i64>> {
        let ids: Vec<i64> = sqlx::query_scalar(
            r#"SELECT source_id FROM document_links
               WHERE target_type = $1 AND target_id = $2 AND source_type = $3
               UNION
               SELECT target_id FROM document_links
               WHERE source_type = $1 AND source_id = $2 AND target_type = $3"#,
        )
        .bind(anchor_type)
        .bind(anchor_id)
        .bind(other_type)
        .fetch_all(&mut *executor)
        .await?;
        Ok(ids)
    }
}
