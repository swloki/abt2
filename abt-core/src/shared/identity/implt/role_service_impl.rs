use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::model::Role;
use super::super::permission_cache::RolePermissionCache;
use super::super::repo::IdentityRepo;
use super::super::role_service::RoleService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

pub struct RoleServiceImpl {
    pool: Arc<PgPool>,
    cache: Arc<RolePermissionCache>,
}

impl RoleServiceImpl {
    pub fn new(pool: Arc<PgPool>, cache: Arc<RolePermissionCache>) -> Self {
        Self { pool, cache }
    }
}

#[async_trait]
impl RoleService for RoleServiceImpl {
    async fn create_role(
        &self,
        ctx: ServiceContext<'_>,
        role_name: &str,
        role_code: &str,
        description: Option<&str>,
        parent_role_id: Option<i64>,
    ) -> Result<Role, DomainError> {
        let role = IdentityRepo::insert_role(
            &mut *ctx.executor,
            role_name,
            role_code,
            description,
            parent_role_id,
        )
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                DomainError::duplicate("Role with this code")
            } else {
                DomainError::Internal(e.into())
            }
        })?;

        Ok(role)
    }

    async fn update_role(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
        role_name: &str,
        description: Option<&str>,
    ) -> Result<Role, DomainError> {
        let role = IdentityRepo::update_role(&mut *ctx.executor, role_id, role_name, description)
            .await
            .map_err(|e| {
                if is_no_row(&e) {
                    DomainError::not_found("Role")
                } else {
                    DomainError::Internal(e.into())
                }
            })?;

        Ok(role)
    }

    async fn delete_role(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
    ) -> Result<(), DomainError> {
        IdentityRepo::delete_role(&mut *ctx.executor, role_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // Reload permission cache after role deletion
        let pool = self.pool.clone();
        self.cache.reload(&pool).await?;

        Ok(())
    }

    async fn list_roles(
        &self,
        ctx: ServiceContext<'_>,
    ) -> Result<Vec<Role>, DomainError> {
        IdentityRepo::list_roles(&mut *ctx.executor)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn assign_permissions(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<(), DomainError> {
        IdentityRepo::assign_permissions(&mut *ctx.executor, role_id, &permissions)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // Reload permission cache after permission change
        let pool = self.pool.clone();
        self.cache.reload(&pool).await?;

        Ok(())
    }

    async fn remove_permissions(
        &self,
        ctx: ServiceContext<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<(), DomainError> {
        IdentityRepo::remove_permissions(&mut *ctx.executor, role_id, &permissions)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // Reload permission cache after permission change
        let pool = self.pool.clone();
        self.cache.reload(&pool).await?;

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
