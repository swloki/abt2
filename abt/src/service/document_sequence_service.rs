use anyhow::Result;
use async_trait::async_trait;

use crate::repositories::Executor;

#[async_trait]
pub trait DocumentSequenceService: Send + Sync {
    async fn next_number(&self, executor: Executor<'_>, doc_type: &str) -> Result<String>;
}
