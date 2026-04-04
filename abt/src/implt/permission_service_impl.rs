use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::models::*;
use crate::repositories::{DepartmentResourceAccessRepo, PermissionRepo};
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
    /// 获取用户的所有权限 (新 schema: resource_code:action_code)
    /// super_admin gets all resource permissions expanded from code registry.
    /// Other users get role permissions filtered by department-accessible resources.
    async fn get_user_permissions(&self, user_id: i64) -> Result<Vec<String>> {
        if PermissionRepo::is_super_admin(self.pool.as_ref(), user_id).await? {
            return Ok(crate::collect_all_resources()
                .iter()
                .map(|r| format!("{}:{}", r.resource_code, r.action))
                .collect());
        }
        let role_perms = PermissionRepo::get_user_permission_codes(self.pool.as_ref(), user_id).await?;
        let accessible = DepartmentResourceAccessRepo::resolve_user_accessible_resources(
            self.pool.as_ref(), user_id,
        ).await.unwrap_or_default();
        Ok(DepartmentResourceAccessRepo::filter_permissions_by_department(role_perms, &accessible))
    }

    /// 检查用户是否有某个权限
    /// Applies department filtering: system resources bypass, business resources
    /// require the resource code to be in the user's department-accessible set.
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

        // System resources bypass department filtering (R4)
        if crate::models::is_system_resource(resource_code) {
            return PermissionRepo::check_permission(
                self.pool.as_ref(), user_id, resource_code, action_code,
            ).await;
        }

        // Business resources: check department access first
        if crate::models::is_business_resource(resource_code) {
            let accessible = DepartmentResourceAccessRepo::resolve_user_accessible_resources(
                self.pool.as_ref(), user_id,
            ).await.unwrap_or_default();

            if !accessible.contains(resource_code) {
                return Ok(false); // department doesn't have access to this resource
            }
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
