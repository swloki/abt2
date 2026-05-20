//! 单据序号服务接口
//!
//! 定义单据序号的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;

use crate::repositories::Executor;

/// 单据序号服务接口
#[async_trait]
pub trait DocumentSequenceService: Send + Sync {
    /// 获取下一个单据编号
    async fn next_number(&self, executor: Executor<'_>, doc_type: &str) -> Result<String>;
}
