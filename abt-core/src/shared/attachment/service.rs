use async_trait::async_trait;

use crate::shared::types::{PgExecutor, Result, ServiceContext};

use super::model::{Attachment, AttachmentMeta};

/// 通用附件服务（按 owner_type + owner_id 多态）。
/// 文件字节由通用上传端点（abt-web components/image_upload）即时落盘，
/// 本 service 负责 DB 记录的建立（link）/ 查询（list）/ 删除（delete）。
#[async_trait]
pub trait AttachmentService: Send + Sync {
    /// 按「已上传图片」元信息清单建立附件记录（文件已落盘）。
    /// 用于新建单据提交时把 attachments_json 关联到新单据。
    async fn link(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        owner_type: &str,
        owner_id: i64,
        metas: Vec<AttachmentMeta>,
    ) -> Result<()>;

    /// 列出某单据的全部附件（按上传时间）。
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        owner_type: &str,
        owner_id: i64,
    ) -> Result<Vec<Attachment>>;

    /// 删除附件（事务内删记录）→ 返回含 stored_path 的实体，调用方 commit 后删文件。
    async fn delete(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        attachment_id: i64,
    ) -> Result<Attachment>;
}
