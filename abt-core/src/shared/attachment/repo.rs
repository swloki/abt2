use crate::shared::types::{PgExecutor, Result};

use super::model::{Attachment, CreateAttachmentParams};

const ATTACHMENT_COLUMNS: &str =
    "id, owner_type, owner_id, file_name, stored_path, content_type, file_size, operator_id, created_at";

pub struct AttachmentRepo;

impl AttachmentRepo {
    pub async fn insert(
        &self,
        executor: PgExecutor<'_>,
        params: &CreateAttachmentParams<'_>,
    ) -> Result<Attachment> {
        let row = sqlx::query_as::<sqlx::Postgres, Attachment>(sqlx::AssertSqlSafe(format!(
            "INSERT INTO attachments (owner_type, owner_id, file_name, stored_path, content_type, file_size, operator_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING {ATTACHMENT_COLUMNS}"
        )))
        .bind(params.owner_type)
        .bind(params.owner_id)
        .bind(params.file_name)
        .bind(params.stored_path)
        .bind(params.content_type)
        .bind(params.file_size)
        .bind(params.operator_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn list_by_owner(
        &self,
        executor: PgExecutor<'_>,
        owner_type: &str,
        owner_id: i64,
    ) -> Result<Vec<Attachment>> {
        let rows = sqlx::query_as::<sqlx::Postgres, Attachment>(sqlx::AssertSqlSafe(format!(
            "SELECT {ATTACHMENT_COLUMNS} FROM attachments WHERE owner_type = $1 AND owner_id = $2 ORDER BY created_at"
        )))
        .bind(owner_type)
        .bind(owner_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_id(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<Attachment>> {
        let row = sqlx::query_as::<sqlx::Postgres, Attachment>(sqlx::AssertSqlSafe(format!(
            "SELECT {ATTACHMENT_COLUMNS} FROM attachments WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM attachments WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }
}
