use anyhow::Result;
use async_trait::async_trait;

use crate::models::{
    CreateRoleRequest, Role, RoleWithPermissions, UpdateRoleRequest,
};
use crate::repositories::Executor;

#[async_trait]
pub trait RoleService: Send + Sync {
    async fn create(
        &self,
        operator_id: Option<i64>,
        req: CreateRoleRequest,
        executor: Executor<'_>,
    ) -> Result<i64>;

    async fn update(
        &self,
        operator_id: Option<i64>,
        role_id: i64,
        req: UpdateRoleRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn delete(
        &self,
        operator_id: Option<i64>,
        role_id: i64,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn get(&self, role_id: i64) -> Result<Option<RoleWithPermissions>>;

    async fn list(&self) -> Result<Vec<Role>>;

    async fn assign_permissions(
        &self,
        operator_id: Option<i64>,
        role_id: i64,
        resource_actions: Vec<(String, String)>,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn remove_permissions(
        &self,
        operator_id: Option<i64>,
        role_id: i64,
        resource_actions: Vec<(String, String)>,
        executor: Executor<'_>,
    ) -> Result<()>;
}
