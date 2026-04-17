use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::models::*;
use crate::repositories::PermissionRepo;
use crate::service::PermissionService;

pub struct PermissionServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl PermissionServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PermissionService for PermissionServiceImpl {
    /// 获取用户的所有权限 (resource_code:action_code)
    /// super_admin gets all resource permissions expanded from code registry.
    /// Other users get role permissions from user_roles via RolePermissionCache.
    async fn get_user_permissions(&self, user_id: i64) -> Result<Vec<String>> {
        if PermissionRepo::is_super_admin(self.pool.as_ref(), user_id).await? {
            return Ok(crate::collect_all_resources()
                .iter()
                .map(|r| format!("{}:{}", r.resource_code, r.action))
                .collect());
        }
        PermissionRepo::get_user_permission_codes(self.pool.as_ref(), user_id).await
    }

    /// 检查用户是否有某个权限
    async fn check_permission(
        &self,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool> {
        // super_admin always has permission
        if PermissionRepo::is_super_admin(self.pool.as_ref(), user_id).await? {
            return Ok(true);
        }

        PermissionRepo::check_permission(
            self.pool.as_ref(), user_id, resource_code, action_code,
        ).await
    }

    /// 获取审计日志
    async fn list_audit_logs(&self, limit: i64, offset: i64) -> Result<Vec<AuditLog>> {
        PermissionRepo::list_audit_logs(self.pool.as_ref(), limit, offset).await
    }
}
