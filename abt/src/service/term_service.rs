//! 分类服务接口
//!
//! 定义分类管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{CreateTermRequest, Term, TermTree, UpdateTermRequest};
use crate::repositories::Executor;

/// 分类服务接口
#[async_trait]
pub trait TermService: Send + Sync {
    /// 创建新分类
    async fn create(&self, request: CreateTermRequest, executor: Executor<'_>) -> Result<i64>;

    /// 更新分类
    async fn update(
        &self,
        term_id: i64,
        request: UpdateTermRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 删除分类
    async fn delete(&self, term_id: i64, executor: Executor<'_>) -> Result<()>;

    /// 根据 ID 查找分类
    async fn find(&self, term_id: i64) -> Result<Option<Term>>;

    /// 获取分类树
    async fn get_tree(&self, taxonomy: &str) -> Result<Vec<TermTree>>;

    /// 获取子分类
    async fn get_children(&self, parent_id: i64) -> Result<Vec<Term>>;

    /// 获取指定分类法的所有分类
    async fn list_by_taxonomy(&self, taxonomy: &str) -> Result<Vec<Term>>;
}
