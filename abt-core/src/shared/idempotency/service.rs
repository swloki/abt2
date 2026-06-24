use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

use super::super::types::context::ServiceContext;
use super::super::types::{PgExecutor, Result};

#[async_trait]
pub trait IdempotencyService: Send + Sync {
    /// 检查并标记事件处理中。
    /// - true = 首次处理，应继续业务逻辑
    /// - false = 重复事件，应跳过
    async fn check_and_mark(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        event_id: i64,
        handler_name: &str,
    ) -> Result<bool>;

    /// 标记事件处理完成，存储可选结果
    async fn mark_processed(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        event_id: i64,
        handler_name: &str,
        result: Option<JsonValue>,
    ) -> Result<()>;

    /// 清理过期的幂等记录，返回删除条数
    async fn cleanup_expired(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        before: DateTime<Utc>,
    ) -> Result<u64>;

    /// HTTP 请求级幂等：尝试声明一个 key（纯 INSERT ON CONFLICT，不重置状态）。
    /// - true = 首次（应继续业务）
    /// - false = 重复（应幂等返回，跳过业务）
    /// 适合并发请求（双击/网络重试），区别于 check_and_mark 的事件处理幂等（会重置 Processing 残留）。
    /// 记录带 expires_at，由 cleanup_expired 清理。
    async fn try_claim(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        key: &str,
    ) -> Result<bool>;
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
