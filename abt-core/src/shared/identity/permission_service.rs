use async_trait::async_trait;

use crate::shared::types::Result;

#[async_trait]
pub trait PermissionService: Send + Sync {
    /// Check if any of the given roles has the specified permission.
    /// Super admins always pass.
    async fn check_permission(
        &self,
        is_super_admin: bool,
        role_ids: &[i64],
        resource: &str,
        action: &str,
    ) -> Result<bool>;

    /// Batch check multiple (resource, action) pairs.
    /// Super admins get all true.
    async fn batch_check_permissions(
        &self,
        is_super_admin: bool,
        role_ids: &[i64],
        pairs: &[(String, String)],
    ) -> Result<Vec<bool>>;

    /// Get all resolved permission strings for the given roles.
    async fn get_user_permissions(
        &self,
        role_ids: &[i64],
    ) -> Result<Vec<String>>;
}
