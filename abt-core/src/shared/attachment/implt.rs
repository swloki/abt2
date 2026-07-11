use async_trait::async_trait;
use sqlx::PgPool;

use crate::shared::types::{DomainError, PgExecutor, Result, ServiceContext};

use super::model::{Attachment, AttachmentMeta, CreateAttachmentParams};
use super::repo::AttachmentRepo;
use super::service::AttachmentService;

pub struct AttachmentServiceImpl {
    repo: AttachmentRepo,
}

impl AttachmentServiceImpl {
    pub fn new(_pool: PgPool) -> Self {
        Self { repo: AttachmentRepo }
    }
}

#[async_trait]
impl AttachmentService for AttachmentServiceImpl {
    async fn link(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        owner_type: &str,
        owner_id: i64,
        metas: Vec<AttachmentMeta>,
    ) -> Result<()> {
        for m in &metas {
            self.repo
                .insert(
                    db,
                    &CreateAttachmentParams {
                        owner_type,
                        owner_id,
                        file_name: &m.name,
                        stored_path: &m.path,
                        content_type: &m.content_type,
                        file_size: m.size,
                        operator_id: ctx.operator_id,
                    },
                )
                .await?;
        }
        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        owner_type: &str,
        owner_id: i64,
    ) -> Result<Vec<Attachment>> {
        self.repo.list_by_owner(db, owner_type, owner_id).await
    }

    async fn delete(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        attachment_id: i64,
    ) -> Result<Attachment> {
        let attachment = self
            .repo
            .find_by_id(db, attachment_id)
            .await?
            .ok_or_else(|| DomainError::not_found("Attachment"))?;
        self.repo.delete(db, attachment_id).await?;
        Ok(attachment)
    }
}
