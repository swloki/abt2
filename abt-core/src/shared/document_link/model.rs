use chrono::{DateTime, Utc};

use crate::shared::enums::{DocumentType, LinkType};

/// 单据关联实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DocumentLink {
    pub id: i64,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub target_type: DocumentType,
    pub target_id: i64,
    pub link_type: LinkType,
    pub path: String,
    pub depth: i32,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<i64>,
}

/// 创建关联请求
#[derive(Debug, Clone)]
pub struct LinkRequest {
    pub source_type: DocumentType,
    pub source_id: i64,
    pub target_type: DocumentType,
    pub target_id: i64,
    pub link_type: LinkType,
}
