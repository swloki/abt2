use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher, PasswordHash, PasswordVerifier};

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
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?;
        Ok(hash.to_string())
    }

    async fn log_audit(executor: Executor<'_>, entry: AuditEntry) -> Result<()> {
        PermissionRepo::insert_audit_log(executor, &entry).await
    }
}

#[async_trait]
impl UserService for UserServiceImpl {
    async fn create(&self, operator_id: Option<i64>, req: CreateUserRequest, executor: Executor<'_>) -> Result<i64> {
        let password_hash = Self::hash_password(&req.password)?;
        let user_id = UserRepo::insert(executor, &req, &password_hash).await?;
        Self::log_audit(executor, AuditEntry {
            operator_id,
            target_type: "user",
            target_id: user_id,
            action: "create",
            old_value: None,
            new_value: Some(serde_json::to_value(&req)?),
        }).await?;
        Ok(user_id)
    }

    async fn update(&self, operator_id: Option<i64>, user_id: i64, req: UpdateUserRequest, executor: Executor<'_>) -> Result<()> {
        let old_user = UserRepo::find_by_id_with_executor(executor, user_id).await?.ok_or_else(|| anyhow!("User not found"))?;
        UserRepo::update(executor, user_id, &req).await?;
        Self::log_audit(executor, AuditEntry {
            operator_id,
            target_type: "user",
            target_id: user_id,
            action: "update",
            old_value: Some(serde_json::to_value(&old_user)?),
            new_value: Some(serde_json::to_value(&req)?),
        }).await?;
        Ok(())
    }

    async fn delete(&self, operator_id: Option<i64>, user_id: i64, executor: Executor<'_>) -> Result<()> {
        let old_user = UserRepo::find_by_id_with_executor(executor, user_id).await?.ok_or_else(|| anyhow!("User not found"))?;
        UserRepo::delete(executor, user_id).await?;
        Self::log_audit(executor, AuditEntry {
            operator_id,
            target_type: "user",
            target_id: user_id,
            action: "delete",
            old_value: Some(serde_json::to_value(&old_user)?),
            new_value: None,
        }).await?;
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

    async fn get_users_by_ids(&self, user_ids: Vec<i64>) -> Result<Vec<UserWithRoles>> {
        let users = UserRepo::find_by_ids(self.pool.as_ref(), &user_ids).await?;
        let mut result = Vec::new();
        for user in users {
            let roles = UserRepo::get_user_roles(self.pool.as_ref(), user.user_id).await?;
            result.push(UserWithRoles { user, roles });
        }
        Ok(result)
    }

    async fn assign_roles(&self, operator_id: Option<i64>, user_id: i64, role_ids: Vec<i64>, executor: Executor<'_>) -> Result<()> {
        UserRepo::assign_roles(executor, user_id, &role_ids).await?;
        Self::log_audit(executor, AuditEntry {
            operator_id,
            target_type: "user",
            target_id: user_id,
            action: "assign_roles",
            old_value: None,
            new_value: Some(serde_json::to_value(&role_ids)?),
        }).await?;
        Ok(())
    }

    async fn remove_roles(&self, operator_id: Option<i64>, user_id: i64, role_ids: Vec<i64>, executor: Executor<'_>) -> Result<()> {
        UserRepo::remove_roles(executor, user_id, &role_ids).await?;
        Self::log_audit(executor, AuditEntry {
            operator_id,
            target_type: "user",
            target_id: user_id,
            action: "remove_roles",
            old_value: Some(serde_json::to_value(&role_ids)?),
            new_value: None,
        }).await?;
        Ok(())
    }

    async fn batch_assign_roles(&self, operator_id: Option<i64>, user_ids: Vec<i64>, role_ids: Vec<i64>, executor: Executor<'_>) -> Result<()> {
        UserRepo::batch_assign_roles(executor, &user_ids, &role_ids).await?;
        Self::log_audit(executor, AuditEntry {
            operator_id,
            target_type: "user",
            target_id: 0,
            action: "batch_assign_roles",
            old_value: None,
            new_value: Some(serde_json::json!({"user_ids": user_ids, "role_ids": role_ids})),
        }).await?;
        Ok(())
    }

    async fn change_password(&self, user_id: i64, old_password: &str, new_password: &str) -> Result<()> {
        let user = UserRepo::find_by_id(self.pool.as_ref(), user_id)
            .await?
            .ok_or_else(|| anyhow!("用户不存在"))?;

        // 验证旧密码
        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| anyhow!("密码格式错误"))?;
        Argon2::default()
            .verify_password(old_password.as_bytes(), &parsed_hash)
            .map_err(|_| anyhow!("旧密码不正确"))?;

        // 生成新密码哈希并更新
        let new_hash = Self::hash_password(new_password)?;
        let mut tx = self.pool.begin().await?;
        UserRepo::update_password(&mut tx, user_id, &new_hash).await?;
        tx.commit().await?;

        Ok(())
    }
}
