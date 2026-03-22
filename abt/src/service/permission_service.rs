use anyhow::Result;
use async_trait::async_trait;

use crate::models::{
    AuditLog, PermissionGroup, PermissionInfo, ResourceGroup,
};

#[async_trait]
pub trait PermissionService: Send + Sync {
    /// 获取用户的所有权限
    async fn get_user_permissions(&self, user_id: i64) -> Result<Vec<PermissionInfo>>;

    /// 检查用户是否有某个权限
    async fn check_permission(
        &self,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool>;

    /// 获取资源列表（按分组）
    async fn list_resources(&self) -> Result<Vec<ResourceGroup>>;

    /// 获取所有权限（按分组）
    async fn list_permissions(&self) -> Result<Vec<PermissionGroup>>;

    /// 获取审计日志
    async fn list_audit_logs(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>>;
}
