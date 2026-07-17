use chrono::{DateTime, Utc};
use serde::Deserialize;

/// 通用附件实体（元信息；字节存文件系统）。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Attachment {
    pub id: i64,
    pub owner_type: String,
    pub owner_id: i64,
    pub file_name: String,
    pub stored_path: String,
    pub content_type: String,
    pub file_size: i64,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

/// 新增附件参数（repo 层）。
pub struct CreateAttachmentParams<'a> {
    pub owner_type: &'a str,
    pub owner_id: i64,
    pub file_name: &'a str,
    pub stored_path: &'a str,
    pub content_type: &'a str,
    pub file_size: i64,
    pub operator_id: i64,
}

/// 已上传附件的元信息（新建表单提交时携带，关联到新创建的单据）。
/// 对应通用上传控件即时上传后、提交前累积的图片清单。
#[derive(Debug, Clone, Deserialize)]
pub struct AttachmentMeta {
    pub path: String, // stored_path，如 quotation/{uuid}.png
    pub name: String, // 原始文件名
    #[serde(rename = "type")]
    pub content_type: String, // image/png 等
    pub size: i64,
}
