use async_trait::async_trait;
use serde_json::Value as JsonValue;

use super::model::AuditLogQuery;
use super::super::enums::audit::AuditAction;
use super::super::types::context::ServiceContext;
use super::super::types::Result;
use super::super::types::pagination::PaginatedResult;

#[async_trait]
pub trait AuditLogService: Send + Sync {
    /// 在调用方事务内写入审计日志。
    /// changes 中标记 `sensitive: true` 的字段值自动脱敏为 "***"。
    /// 返回生成的审计日志 id。
    async fn record(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        action: AuditAction,
        changes: Option<JsonValue>,
        context: Option<JsonValue>,
    ) -> Result<i64>;

    /// 分页查询审计日志，支持 entity_type / operator_id / action / time_range 过滤
    async fn query_logs(
        &self,
        ctx: ServiceContext<'_>,
        query: AuditLogQuery,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<AuditLog>>;
}

// Re-export model for trait consumers
pub use super::model::AuditLog;
