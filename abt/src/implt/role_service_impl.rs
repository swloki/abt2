use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use crate::models::*;
use crate::repositories::{Executor, PermissionRepo, RoleRepo};
use crate::service::RoleService;

pub struct RoleServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl RoleServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }

    async fn log_audit(
        &self,
        executor: Executor<'_>,
        operator_id: Option<i64>,
        target_type: &str,
        target_id: i64,
        action: &str,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    ) -> Result<()> {
        PermissionRepo::insert_audit_log(
            executor, operator_id, target_type, target_id, action, old_value, new_value,
        )
        .await
    }
}

#[async_trait]
impl RoleService for RoleServiceImpl {
    async fn create(&self, operator_id: Option<i64>, req: CreateRoleRequest, executor: Executor<'_>) -> Result<i64> {
        let role_id = RoleRepo::insert(executor, &req).await?;
        self.log_audit(executor, operator_id, "role", role_id, "create", None, Some(serde_json::to_value(&req)?)).await?;
        Ok(role_id)
    }

    async fn update(&self, operator_id: Option<i64>, role_id: i64, req: UpdateRoleRequest, executor: Executor<'_>) -> Result<()> {
        let old_role = RoleRepo::find_by_id_with_executor(executor, role_id).await?.ok_or_else(|| anyhow!("Role not found"))?;
        RoleRepo::update(executor, role_id, &req).await?;
        self.log_audit(executor, operator_id, "role", role_id, "update", Some(serde_json::to_value(&old_role)?), Some(serde_json::to_value(&req)?)).await?;
        Ok(())
    }

    async fn delete(&self, operator_id: Option<i64>, role_id: i64, executor: Executor<'_>) -> Result<()> {
        let old_role = RoleRepo::find_by_id_with_executor(executor, role_id).await?.ok_or_else(|| anyhow!("Role not found"))?;
        RoleRepo::delete(executor, role_id).await?;
        self.log_audit(executor, operator_id, "role", role_id, "delete", Some(serde_json::to_value(&old_role)?), None).await?;
        Ok(())
    }

    async fn get(&self, role_id: i64) -> Result<Option<RoleWithPermissions>> {
        let role = RoleRepo::find_by_id(self.pool.as_ref(), role_id).await?;
        match role {
            Some(role) => {
                let permission_codes = RoleRepo::get_role_permission_codes(self.pool.as_ref(), role_id).await?;
                Ok(Some(RoleWithPermissions { role, permissions: permission_codes }))
            }
            None => Ok(None),
        }
    }

    async fn list(&self) -> Result<Vec<Role>> {
        let roles = RoleRepo::list_all(self.pool.as_ref()).await?;
        Ok(roles)
    }

    async fn assign_permissions(&self, operator_id: Option<i64>, role_id: i64, resource_actions: Vec<(String, String)>, executor: Executor<'_>) -> Result<()> {
        RoleRepo::assign_permissions(&mut *executor, role_id, &resource_actions).await?;
        self.log_audit(executor, operator_id, "role", role_id, "assign_permissions", None, Some(serde_json::to_value(&resource_actions)?)).await?;
        Ok(())
    }

    async fn remove_permissions(&self, operator_id: Option<i64>, role_id: i64, resource_actions: Vec<(String, String)>, executor: Executor<'_>) -> Result<()> {
        RoleRepo::remove_permissions(&mut *executor, role_id, &resource_actions).await?;
        self.log_audit(executor, operator_id, "role", role_id, "remove_permissions", Some(serde_json::to_value(&resource_actions)?), None).await?;
        Ok(())
    }
}
