use anyhow::Result;
use async_trait::async_trait;
use crate::repositories::Executor;

#[async_trait]
pub trait DocumentSequenceService: Send + Sync {
    async fn next_number(&self, executor: Executor<'_>, doc_type: &str) -> Result<String>;
    async fn ensure_sequence(&self, executor: Executor<'_>, doc_type: &str, prefix: &str, reset_rule: &str) -> Result<()>;
}
