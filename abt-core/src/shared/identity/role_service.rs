use async_trait::async_trait;

use super::model::{Role, RoleWithPermissions};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

#[async_trait]
pub trait RoleService: Send + Sync {
    async fn create_role(
        &self,
        ctx: ServiceContext<'_>,
        role_name: &str,
        role_code: &str,
        description: Option<&str>,
        parent_role_id: Option<i64>,
    ) -> Result<Role, DomainError>;

    async fn update_role(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
        role_name: &str,
        description: Option<&str>,
    ) -> Result<Role, DomainError>;

    async fn delete_role(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
    ) -> Result<(), DomainError>;

    async fn list_roles(
        &self,
        ctx: ServiceContext<'_>,
    ) -> Result<Vec<Role>, DomainError>;

    async fn assign_permissions(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<(), DomainError>;

    async fn remove_permissions(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<(), DomainError>;

    async fn get_role_with_permissions(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
    ) -> Result<RoleWithPermissions, DomainError>;
}
