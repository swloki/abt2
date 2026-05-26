use async_trait::async_trait;

use super::model::{User, UserWithRoles};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait UserService: Send + Sync {
    async fn create_user(
        &self,
        ctx: ServiceContext<'_>,
        username: &str,
        password: &str,
        display_name: Option<&str>,
        is_super_admin: bool,
    ) -> Result<User>;

    async fn update_user(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        display_name: Option<&str>,
    ) -> Result<User>;

    /// Soft delete: sets is_active = false
    async fn delete_user(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
    ) -> Result<()>;

    async fn get_user(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
    ) -> Result<User>;

    async fn list_users(
        &self,
        ctx: ServiceContext<'_>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<User>>;

    async fn batch_assign_roles(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        role_ids: Vec<i64>,
    ) -> Result<()>;

    async fn get_user_with_roles(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
    ) -> Result<UserWithRoles>;

    async fn list_users_with_roles(
        &self,
        ctx: ServiceContext<'_>,
    ) -> Result<Vec<UserWithRoles>>;

    async fn get_users_by_ids(
        &self,
        ctx: ServiceContext<'_>,
        user_ids: Vec<i64>,
    ) -> Result<Vec<UserWithRoles>>;

    async fn assign_roles(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        role_ids: Vec<i64>,
    ) -> Result<()>;

    async fn remove_roles(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        role_ids: Vec<i64>,
    ) -> Result<()>;

    async fn change_password(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        old_password: &str,
        new_password: &str,
    ) -> Result<()>;

    async fn update_user_status(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        is_active: bool,
    ) -> Result<User>;
}
