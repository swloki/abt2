//! 科目映射服务接口
use async_trait::async_trait;

use crate::shared::types::{PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait GlMappingService: Send + Sync {
    /// 解析科目映射：先查产品级（mapping_key+product_id），无则全局默认（mapping_key+product_id IS NULL）
    /// 都没有则返回 MissingAccountMapping 错误
    async fn resolve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        mapping_key: &str,
        product_id: Option<i64>,
    ) -> Result<i64>;
}
