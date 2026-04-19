use anyhow::Result;
use async_trait::async_trait;

use crate::models::{
    CreateUserRequest, UpdateUserRequest, UserWithRoles,
};
use crate::repositories::Executor;

#[async_trait]
pub trait UserService: Send + Sync {
    async fn create(
        &self,
        operator_id: Option<i64>,
        req: CreateUserRequest,
        executor: Executor<'_>,
    ) -> Result<i64>;

    async fn update(
        &self,
        operator_id: Option<i64>,
        user_id: i64,
        req: UpdateUserRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn delete(
        &self,
        operator_id: Option<i64>,
        user_id: i64,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn get(&self, user_id: i64) -> Result<Option<UserWithRoles>>;

    async fn list(&self) -> Result<Vec<UserWithRoles>>;

    async fn get_users_by_ids(&self, user_ids: Vec<i64>) -> Result<Vec<UserWithRoles>>;

    async fn assign_roles(
        &self,
        operator_id: Option<i64>,
        user_id: i64,
        role_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn remove_roles(
        &self,
        operator_id: Option<i64>,
        user_id: i64,
        role_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn batch_assign_roles(
        &self,
        operator_id: Option<i64>,
        user_ids: Vec<i64>,
        role_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()>;
}
