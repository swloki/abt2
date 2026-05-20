use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use crate::repositories::{DocumentSequenceRepo, Executor};
use crate::service::DocumentSequenceService;

pub struct DocumentSequenceServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl DocumentSequenceServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DocumentSequenceService for DocumentSequenceServiceImpl {
    async fn next_number(&self, executor: Executor<'_>, doc_type: &str) -> Result<String> {
        DocumentSequenceRepo::next_number(executor, doc_type).await
    }

    async fn ensure_sequence(&self, executor: Executor<'_>, doc_type: &str, prefix: &str, reset_rule: &str) -> Result<()> {
        DocumentSequenceRepo::ensure_sequence(executor, doc_type, prefix, reset_rule).await
    }
}
