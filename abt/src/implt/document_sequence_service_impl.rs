//! 单据序号服务实现
//!
//! 实现单据序号的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::repositories::DocumentSequenceRepo;
use crate::repositories::Executor;
use crate::service::DocumentSequenceService;

/// 单据序号服务实现
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
}
