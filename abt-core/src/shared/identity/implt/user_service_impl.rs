use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::super::model::{User, UserWithRoles};
use super::super::repo::IdentityRepo;
use super::super::user_service::UserService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::audit_log::new_audit_log_service;
use crate::shared::types::PgExecutor;
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

pub struct UserServiceImpl {
    pool: PgPool,
}

impl UserServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        username: &str,
        password: &str,
        display_name: Option<&str>,
        is_super_admin: bool,
    ) -> Result<User> {
        let username = username.trim();
        if username.is_empty() || username.len() > 64 {
            return Err(DomainError::Validation(
                "Username must be 1-64 characters".to_string(),
            ));
        }
        if password.len() < 8 {
            return Err(DomainError::Validation(
                "Password must be at least 8 characters".to_string(),
            ));
        }

        let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| DomainError::Internal(anyhow::anyhow!("argon2 hash error: {e}")))?
            .to_string();

        let user = IdentityRepo::insert_user(
            &mut *db,
            username,
            &password_hash,
            display_name,
            is_super_admin,
        )
        .await
        .map_err(|e| match &e { DomainError::Internal(inner) if is_unique_violation(inner) => DomainError::duplicate("User with this username"), _ => e })?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "user",
                user.user_id,
                AuditAction::Create,
                Some(json!({
                    "username": { "old": null, "new": username },
                    "display_name": { "old": null, "new": display_name },
                    "is_super_admin": { "old": null, "new": is_super_admin },
                })),
                None,
            )
            .await?;

        Ok(user)
    }

    async fn update_user(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        display_name: Option<&str>,
    ) -> Result<User> {
        let user = IdentityRepo::update_user(&mut *db, user_id, display_name)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "user",
                user_id,
                AuditAction::Update,
                Some(json!({
                    "display_name": { "new": display_name }
                })),
                None,
            )
            .await?;

        Ok(user)
    }

    async fn delete_user(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
    ) -> Result<()> {
        IdentityRepo::deactivate_user(&mut *db, user_id)
            .await
            ?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, "user", user_id, AuditAction::Delete, None, None)
            .await?;

        Ok(())
    }

    async fn get_user(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
    ) -> Result<User> {
        IdentityRepo::get_user(&mut *db, user_id)
            .await
            .map_err(|e| match &e {
                DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"),
                _ => e,
            })
    }

    async fn list_users(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<User>> {
        let params = crate::shared::types::pagination::PageParams::new(page, page_size);
        let (items, total) = IdentityRepo::list_users(
            &mut *db,
            params.page_size.into(),
            params.offset().into(),
        )
        .await
        ?;

        Ok(PaginatedResult::new(items, total as u64, params.page, params.page_size))
    }

    async fn batch_assign_roles(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        role_ids: Vec<i64>,
    ) -> Result<()> {
        IdentityRepo::get_user(&mut *db, user_id)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        IdentityRepo::replace_user_roles(&mut *db, user_id, &role_ids)
            .await
            ?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "user",
                user_id,
                AuditAction::Update,
                Some(json!({
                    "role_ids": { "new": role_ids }
                })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn get_user_with_roles(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
    ) -> Result<UserWithRoles> {
        let user = IdentityRepo::get_user(&mut *db, user_id)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        let roles = IdentityRepo::get_role_info_for_user(&mut *db, user_id)
            .await
            ?;

        Ok(UserWithRoles { user, roles })
    }

    async fn list_users_with_roles(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
    ) -> Result<Vec<UserWithRoles>> {
        let (users, _total) = IdentityRepo::list_users(
            &mut *db,
            i64::MAX,
            0,
        )
        .await
        ?;

        let mut result = Vec::with_capacity(users.len());
        for user in users {
            let roles = IdentityRepo::get_role_info_for_user(&mut *db, user.user_id)
                .await
                ?;
            result.push(UserWithRoles { user, roles });
        }

        Ok(result)
    }

    async fn get_users_by_ids(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        user_ids: Vec<i64>,
    ) -> Result<Vec<UserWithRoles>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let users = IdentityRepo::get_users_by_ids(&mut *db, &user_ids)
            .await
            ?;

        let mut result = Vec::with_capacity(users.len());
        for user in users {
            let roles = IdentityRepo::get_role_info_for_user(&mut *db, user.user_id)
                .await
                ?;
            result.push(UserWithRoles { user, roles });
        }

        Ok(result)
    }

    async fn assign_roles(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        role_ids: Vec<i64>,
    ) -> Result<()> {
        IdentityRepo::get_user(&mut *db, user_id)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        IdentityRepo::add_user_roles(&mut *db, user_id, &role_ids)
            .await
            ?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "user",
                user_id,
                AuditAction::Update,
                Some(json!({
                    "assign_role_ids": { "new": role_ids }
                })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn remove_roles(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        role_ids: Vec<i64>,
    ) -> Result<()> {
        IdentityRepo::get_user(&mut *db, user_id)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        IdentityRepo::remove_user_roles(&mut *db, user_id, &role_ids)
            .await
            ?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "user",
                user_id,
                AuditAction::Update,
                Some(json!({
                    "remove_role_ids": { "new": role_ids }
                })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn change_password(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        old_password: &str,
        new_password: &str,
    ) -> Result<()> {
        // Fetch stored password hash
        let stored_hash = IdentityRepo::get_user_password_hash(&mut *db, user_id)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        // Verify old password
        let parsed_hash = PasswordHash::new(&stored_hash)
            .map_err(|e| DomainError::Internal(anyhow::anyhow!("argon2 parse error: {e}")))?;
        Argon2::default()
            .verify_password(old_password.as_bytes(), &parsed_hash)
            .map_err(|_| DomainError::permission_denied("Old password is incorrect"))?;

        // Validate new password
        if new_password.len() < 8 {
            return Err(DomainError::Validation(
                "New password must be at least 8 characters".to_string(),
            ));
        }

        // Hash new password
        let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let new_hash = Argon2::default()
            .hash_password(new_password.as_bytes(), &salt)
            .map_err(|e| DomainError::Internal(anyhow::anyhow!("argon2 hash error: {e}")))?
            .to_string();

        IdentityRepo::update_user_password(&mut *db, user_id, &new_hash)
            .await
            ?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "user",
                user_id,
                AuditAction::Update,
                Some(json!({ "password_changed": true })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn update_user_status(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        is_active: bool,
    ) -> Result<User> {
        let user = IdentityRepo::update_user_status(&mut *db, user_id, is_active)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "user",
                user_id,
                AuditAction::Update,
                Some(json!({
                    "is_active": { "new": is_active }
                })),
                None,
            )
            .await?;

        Ok(user)
    }
}

fn is_unique_violation(err: &anyhow::Error) -> bool {
    err.downcast_ref::<sqlx::Error>()
        .map(|e| if let sqlx::Error::Database(db_err) = e {
            db_err.code().as_ref().map(|c| c == "23505").unwrap_or(false)
        } else {
            false
        })
        .unwrap_or(false)
}

fn is_no_row(err: &anyhow::Error) -> bool {
    err.downcast_ref::<sqlx::Error>()
        .map(|e| matches!(e, sqlx::Error::RowNotFound))
        .unwrap_or(false)
}
