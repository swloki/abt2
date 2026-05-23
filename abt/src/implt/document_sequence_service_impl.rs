use anyhow::Result;
use async_trait::async_trait;

use crate::repositories::{DocumentSequenceRepo, Executor};
use crate::service::DocumentSequenceService;

#[derive(Default)]
pub struct DocumentSequenceServiceImpl;

impl DocumentSequenceServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DocumentSequenceService for DocumentSequenceServiceImpl {
    async fn next_number(&self, executor: Executor<'_>, doc_type: &str) -> Result<String> {
        DocumentSequenceRepo::ensure_sequence(&mut *executor, doc_type, doc_type, "monthly").await?;
        DocumentSequenceRepo::next_number(&mut *executor, doc_type).await
    }
}
