use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use crate::models::*;
use crate::repositories::{Executor, PermissionRepo, UserRepo};
use crate::service::UserService;

pub struct UserServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl UserServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }

    fn hash_password(password: &str) -> Result<String> {
        Ok(format!("hashed:{}", password))
    }

    async fn log_audit(
        &self,
        executor: Executor<'_>,
        operator_id: i64,
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
impl UserService for UserServiceImpl {
    async fn create(&self, operator_id: i64, req: CreateUserRequest, executor: Executor<'_>) -> Result<i64> {
        let password_hash = Self::hash_password(&req.password)?;
        let user_id = UserRepo::insert(executor, &req, &password_hash).await?;
        self.log_audit(executor, operator_id, "user", user_id, "create", None, Some(serde_json::to_value(&req)?)).await?;
        Ok(user_id)
    }

    async fn update(&self, operator_id: i64, user_id: i64, req: UpdateUserRequest, executor: Executor<'_>) -> Result<()> {
        let old_user = UserRepo::find_by_id_with_executor(executor, user_id).await?.ok_or_else(|| anyhow!("User not found"))?;
        UserRepo::update(executor, user_id, &req).await?;
        self.log_audit(executor, operator_id, "user", user_id, "update", Some(serde_json::to_value(&old_user)?), Some(serde_json::to_value(&req)?)).await?;
        Ok(())
    }

    async fn delete(&self, operator_id: i64, user_id: i64, executor: Executor<'_>) -> Result<()> {
        let old_user = UserRepo::find_by_id_with_executor(executor, user_id).await?.ok_or_else(|| anyhow!("User not found"))?;
        UserRepo::delete(executor, user_id).await?;
        self.log_audit(executor, operator_id, "user", user_id, "delete", Some(serde_json::to_value(&old_user)?), None).await?;
        Ok(())
    }

    async fn get(&self, user_id: i64) -> Result<Option<UserWithRoles>> {
        let user = UserRepo::find_by_id(self.pool.as_ref(), user_id).await?;
        match user {
            Some(user) => {
                let roles = UserRepo::get_user_roles(self.pool.as_ref(), user.user_id).await?;
                Ok(Some(UserWithRoles { user, roles }))
            }
            None => Ok(None),
        }
    }

    async fn list(&self) -> Result<Vec<UserWithRoles>> {
        let users = UserRepo::list_all(self.pool.as_ref()).await?;
        let mut result = Vec::new();
        for user in users {
            let roles = UserRepo::get_user_roles(self.pool.as_ref(), user.user_id).await?;
            result.push(UserWithRoles { user, roles });
        }
        Ok(result)
    }

    async fn assign_roles(&self, operator_id: i64, user_id: i64, role_ids: Vec<i64>, executor: Executor<'_>) -> Result<()> {
        UserRepo::assign_roles(executor, user_id, &role_ids).await?;
        self.log_audit(executor, operator_id, "user", user_id, "assign_roles", None, Some(serde_json::to_value(&role_ids)?)).await?;
        Ok(())
    }

    async fn remove_roles(&self, operator_id: i64, user_id: i64, role_ids: Vec<i64>, executor: Executor<'_>) -> Result<()> {
        UserRepo::remove_roles(executor, user_id, &role_ids).await?;
        self.log_audit(executor, operator_id, "user", user_id, "remove_roles", Some(serde_json::to_value(&role_ids)?), None).await?;
        Ok(())
    }

    async fn batch_assign_roles(&self, operator_id: i64, user_ids: Vec<i64>, role_ids: Vec<i64>, executor: Executor<'_>) -> Result<()> {
        UserRepo::batch_assign_roles(executor, &user_ids, &role_ids).await?;
        self.log_audit(executor, operator_id, "user", 0, "batch_assign_roles", None, serde_json::json!({"user_ids": user_ids, "role_ids": role_ids}).into()).await?;
        Ok(())
    }
}
