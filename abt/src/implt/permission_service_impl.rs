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
    async fn get_user_permissions(&self, user_id: i64) -> Result<Vec<PermissionInfo>> {
        let permissions = PermissionRepo::get_user_permissions(self.pool.as_ref(), user_id).await?;
        Ok(permissions)
    }

    async fn check_permission(&self, user_id: i64, resource_code: &str, action_code: &str) -> Result<bool> {
        let has_permission = PermissionRepo::check_permission(self.pool.as_ref(), user_id, resource_code, action_code).await?;
        Ok(has_permission)
    }

    async fn list_resources(&self) -> Result<Vec<ResourceGroup>> {
        let resources = PermissionRepo::list_resources(self.pool.as_ref()).await?;

        // Group resources by group_name
        let mut groups: std::collections::HashMap<String, Vec<Resource>> = std::collections::HashMap::new();
        for resource in resources {
            let group_name = resource.group_name.clone().unwrap_or_default();
            groups.entry(group_name).or_default().push(resource);
        }

        let result: Vec<ResourceGroup> = groups
            .into_iter()
            .map(|(group_name, resources)| ResourceGroup { group_name, resources })
            .collect();

        Ok(result)
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionGroup>> {
        let permissions = PermissionRepo::list_permissions(self.pool.as_ref()).await?;

        // Group permissions by group_name
        let mut groups: std::collections::HashMap<String, Vec<PermissionInfo>> = std::collections::HashMap::new();
        for permission in permissions {
            groups.entry(permission.group_name.clone()).or_default().push(permission);
        }

        let result: Vec<PermissionGroup> = groups
            .into_iter()
            .map(|(group_name, permissions)| PermissionGroup { group_name, permissions })
            .collect();

        Ok(result)
    }

    async fn list_audit_logs(&self, limit: i64, offset: i64) -> Result<Vec<AuditLog>> {
        let logs = PermissionRepo::list_audit_logs(self.pool.as_ref(), limit, offset).await?;
        Ok(logs)
    }
}
