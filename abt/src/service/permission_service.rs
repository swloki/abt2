use anyhow::Result;
use async_trait::async_trait;

use crate::models::AuditLog;

#[async_trait]
pub trait PermissionService: Send + Sync {
    /// 获取用户的所有权限代码 (resource_code:action_code)
    async fn get_user_permissions(&self, user_id: i64) -> Result<Vec<String>>;

    /// 检查用户是否有某个权限
    async fn check_permission(
        &self,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool>;

    /// 获取审计日志
    async fn list_audit_logs(&self, limit: i64, offset: i64) -> Result<Vec<AuditLog>>;
}
