use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

use super::super::types::context::ServiceContext;
use super::super::types::error::DomainError;
use super::super::types::Result;

#[async_trait]
pub trait IdempotencyService: Send + Sync {
    /// 检查并标记事件处理中。
    /// - true = 首次处理，应继续业务逻辑
    /// - false = 重复事件，应跳过
    async fn check_and_mark(
        &self,
        ctx: ServiceContext<'_>,
        event_id: i64,
        handler_name: &str,
    ) -> Result<bool>;

    /// 标记事件处理完成，存储可选结果
    async fn mark_processed(
        &self,
        ctx: ServiceContext<'_>,
        event_id: i64,
        handler_name: &str,
        result: Option<JsonValue>,
    ) -> Result<()>;

    /// 清理过期的幂等记录，返回删除条数
    async fn cleanup_expired(
        &self,
        ctx: ServiceContext<'_>,
        before: DateTime<Utc>,
    ) -> Result<u64>;
}

/// Convert a string idempotency key to i64 for use with check_and_mark.
/// Uses a deterministic hash; collision is negligible in practice.
pub fn key_to_i64(key: &str) -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as i64
}
