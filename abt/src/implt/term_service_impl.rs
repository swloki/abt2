//! 分类服务实现
//!
//! 实现分类管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::{CreateTermRequest, Term, TermTree, UpdateTermRequest};
use crate::repositories::{Executor, TermRepo};
use crate::service::TermService;

/// 分类服务实现
pub struct TermServiceImpl {
    pool: Arc<PgPool>,
}

impl TermServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TermService for TermServiceImpl {
    async fn create(&self, request: CreateTermRequest, executor: Executor<'_>) -> Result<i64> {
        let term_id = TermRepo::insert(
            executor,
            &request.term_name,
            request.term_parent,
            &request.taxonomy,
        )
        .await?;
        Ok(term_id)
    }

    async fn update(
        &self,
        term_id: i64,
        request: UpdateTermRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        TermRepo::update_name(executor, term_id, &request.term_name).await
    }

    async fn delete(&self, term_id: i64, executor: Executor<'_>) -> Result<()> {
        TermRepo::delete(executor, term_id).await
    }

    async fn find(&self, term_id: i64) -> Result<Option<Term>> {
        TermRepo::find_by_id(&self.pool, term_id).await
    }

    async fn get_tree(&self, taxonomy: &str) -> Result<Vec<TermTree>> {
        TermRepo::get_tree(&self.pool, taxonomy).await
    }

    async fn get_children(&self, parent_id: i64) -> Result<Vec<Term>> {
        TermRepo::get_children(&self.pool, parent_id).await
    }

    async fn list_by_taxonomy(&self, taxonomy: &str) -> Result<Vec<Term>> {
        TermRepo::list_by_taxonomy(&self.pool, taxonomy).await
    }
}
