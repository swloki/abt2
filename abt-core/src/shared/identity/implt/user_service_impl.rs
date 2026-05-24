use std::sync::Arc;

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::super::model::User;
use super::super::repo::IdentityRepo;
use super::super::user_service::UserService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

pub struct UserServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    audit: Arc<dyn AuditLogService>,
}

impl UserServiceImpl {
    pub fn new(pool: Arc<PgPool>, audit: Arc<dyn AuditLogService>) -> Self {
        Self { pool, audit }
    }
}

#[async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self,
        ctx: ServiceContext<'_>,
        username: &str,
        password: &str,
        display_name: Option<&str>,
        is_super_admin: bool,
    ) -> Result<User, DomainError> {
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
            &mut *ctx.executor,
            username,
            &password_hash,
            display_name,
            is_super_admin,
        )
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                DomainError::duplicate("User with this username")
            } else {
                DomainError::Internal(e.into())
            }
        })?;

        self.audit
            .record(
                ctx,
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
        ctx: ServiceContext<'_>,
        user_id: i64,
        display_name: Option<&str>,
    ) -> Result<User, DomainError> {
        let user = IdentityRepo::update_user(&mut *ctx.executor, user_id, display_name)
            .await
            .map_err(|e| {
                if is_no_row(&e) {
                    DomainError::not_found("User")
                } else {
                    DomainError::Internal(e.into())
                }
            })?;

        self.audit
            .record(
                ctx,
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
        ctx: ServiceContext<'_>,
        user_id: i64,
    ) -> Result<(), DomainError> {
        IdentityRepo::deactivate_user(&mut *ctx.executor, user_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        self.audit
            .record(ctx, "user", user_id, AuditAction::Delete, None, None)
            .await?;

        Ok(())
    }

    async fn get_user(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
    ) -> Result<User, DomainError> {
        IdentityRepo::get_user(&mut *ctx.executor, user_id)
            .await
            .map_err(|e| {
                if is_no_row(&e) {
                    DomainError::not_found("User")
                } else {
                    DomainError::Internal(e.into())
                }
            })
    }

    async fn list_users(
        &self,
        ctx: ServiceContext<'_>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<User>, DomainError> {
        let params = crate::shared::types::pagination::PageParams::new(page, page_size);
        let (items, total) = IdentityRepo::list_users(
            &mut *ctx.executor,
            params.page_size.into(),
            params.offset().into(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total as u64, params.page, params.page_size))
    }

    async fn batch_assign_roles(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        role_ids: Vec<i64>,
    ) -> Result<(), DomainError> {
        IdentityRepo::get_user(&mut *ctx.executor, user_id)
            .await
            .map_err(|e| {
                if is_no_row(&e) {
                    DomainError::not_found("User")
                } else {
                    DomainError::Internal(e.into())
                }
            })?;

        IdentityRepo::replace_user_roles(&mut *ctx.executor, user_id, &role_ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        self.audit
            .record(
                ctx,
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
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = err {
        db_err.code().as_ref().map(|c| c == "23505").unwrap_or(false)
    } else {
        false
    }
}

fn is_no_row(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::RowNotFound)
}
