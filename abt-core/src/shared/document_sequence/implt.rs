use std::sync::Arc;

use async_trait::async_trait;
use chrono::Datelike;
use sqlx::postgres::PgPool;

use super::repo::DocumentSequenceRepo;
use super::service::DocumentSequenceService;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct DocumentSequenceServiceImpl {
    #[allow(dead_code)] // 保留供未来独立事务模式使用
    pool: Arc<PgPool>,
}

impl DocumentSequenceServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DocumentSequenceService for DocumentSequenceServiceImpl {
    async fn next_number(
        &self,
        ctx: ServiceContext<'_>,
        doc_type: DocumentType,
    ) -> Result<String> {
        // Timestamp 策略：仅 Product 使用
        if matches!(doc_type, DocumentType::Product) {
            let ts = chrono::Utc::now().timestamp();
            return Ok(format!("x{ts}"));
        }

        // Sequential 策略：原子 upsert + 格式化
        let prefix = doc_type.prefix();
        let padding_len: i32 = 5;

        let seq = DocumentSequenceRepo::next_sequential(&mut *ctx.executor, prefix, padding_len)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let year = seq.seq_date.year();
        let month: u32 = seq.seq_date.month();
        let number = format!(
            "{}-{:04}-{:02}-{:0>width$}",
            seq.prefix,
            year,
            month,
            seq.current_value,
            width = seq.padding_len as usize,
        );

        Ok(number)
    }
}
