//! 科目映射服务实现
use sqlx::PgPool;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{DomainError, PgExecutor, Result};

use super::repo::GlMappingRepo;
use super::service::GlMappingService;

pub struct GlMappingServiceImpl {
    _pool: PgPool,
}

impl GlMappingServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait::async_trait]
impl GlMappingService for GlMappingServiceImpl {
    async fn resolve(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        mapping_key: &str,
        product_id: Option<i64>,
    ) -> Result<i64> {
        // 先查产品级映射
        if let Some(pid) = product_id {
            if let Some(mapping) = GlMappingRepo::find_by_key_and_product(db, mapping_key, pid).await? {
                return Ok(mapping.account_id);
            }
        }

        // 再查全局默认
        if let Some(mapping) = GlMappingRepo::find_by_key_default(db, mapping_key).await? {
            return Ok(mapping.account_id);
        }

        // 都没有则返回错误
        Err(DomainError::business_rule("MissingAccountMapping"))
    }
}
