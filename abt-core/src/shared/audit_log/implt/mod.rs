use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sqlx::postgres::PgPool;

use super::repo::AuditLogRepo;
use super::service::{AuditLog, AuditLogService};
use crate::shared::audit_log::model::AuditLogQuery;
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

pub struct AuditLogServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl AuditLogServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

/// 遍历 changes JSON，将标记 `sensitive: true` 的字段值替换为 "***"
fn redact_sensitive(changes: &mut JsonValue) {
    let Some(map) = changes.as_object_mut() else {
        return;
    };
    for (_field_name, field_val) in map.iter_mut() {
        let Some(detail) = field_val.as_object_mut() else {
            continue;
        };
        if let Some(JsonValue::Bool(true)) = detail.get("sensitive") {
            detail.insert("old".into(), JsonValue::String("***".into()));
            detail.insert("new".into(), JsonValue::String("***".into()));
        }
    }
}

#[async_trait]
impl AuditLogService for AuditLogServiceImpl {
    async fn record(
        &self,
        ctx: ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        action: AuditAction,
        mut changes: Option<JsonValue>,
        context: Option<JsonValue>,
    ) -> Result<i64, DomainError> {
        if let Some(ref mut ch) = changes {
            redact_sensitive(ch);
        }

        let id = AuditLogRepo::insert(
            &mut *ctx.executor,
            entity_type,
            entity_id,
            action,
            changes.as_ref(),
            ctx.operator_id,
            context.as_ref(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(id)
    }

    async fn query_logs(
        &self,
        ctx: ServiceContext<'_>,
        query: AuditLogQuery,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<AuditLog>, DomainError> {
        let params = PageParams::new(page, page_size);

        let (items, total) = AuditLogRepo::query(
            &mut *ctx.executor,
            &query,
            params.page_size.into(),
            params.offset().into(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_redact_sensitive_basic() {
        let mut changes = json!({
            "password": { "old": "secret123", "new": "newpass456", "sensitive": true },
            "name": { "old": "Alice", "new": "Bob" }
        });
        redact_sensitive(&mut changes);

        let password = changes["password"].as_object().unwrap();
        assert_eq!(password["old"], "***");
        assert_eq!(password["new"], "***");

        let name = changes["name"].as_object().unwrap();
        assert_eq!(name["old"], "Alice");
        assert_eq!(name["new"], "Bob");
    }

    #[test]
    fn test_redact_sensitive_no_sensitive_fields() {
        let mut changes = json!({ "amount": { "old": 100, "new": 200 } });
        redact_sensitive(&mut changes);
        assert_eq!(changes["amount"]["old"], 100);
        assert_eq!(changes["amount"]["new"], 200);
    }

    #[test]
    fn test_redact_sensitive_non_object() {
        let mut changes = json!("just a string");
        redact_sensitive(&mut changes);
        assert_eq!(changes, json!("just a string"));
    }

    #[test]
    fn test_redact_sensitive_empty_object() {
        let mut changes = json!({});
        redact_sensitive(&mut changes);
        assert!(changes.as_object().unwrap().is_empty());
    }
}
