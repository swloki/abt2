use async_trait::async_trait;

use super::model::{Role, RoleWithPermissions};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

#[async_trait]
pub trait RoleService: Send + Sync {
    async fn create_role(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        role_name: &str,
        role_code: &str,
        description: Option<&str>,
        parent_role_id: Option<i64>,
    ) -> Result<Role>;

    async fn update_role(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
        role_name: &str,
        description: Option<&str>,
    ) -> Result<Role>;

    async fn delete_role(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
    ) -> Result<()>;

    async fn list_roles(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
    ) -> Result<Vec<Role>>;

    async fn assign_permissions(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<()>;

    async fn remove_permissions(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<()>;

    async fn get_role_with_permissions(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
    ) -> Result<RoleWithPermissions>;
}
