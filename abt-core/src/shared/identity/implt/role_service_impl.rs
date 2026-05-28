use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::model::{Role, RoleWithPermissions};
use super::super::permission_cache::RolePermissionCache;
use super::super::repo::IdentityRepo;
use super::super::role_service::RoleService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

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
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        role_name: &str,
        role_code: &str,
        description: Option<&str>,
        parent_role_id: Option<i64>,
    ) -> Result<Role> {
        let role = IdentityRepo::insert_role(
            &mut *db,
            role_name,
            role_code,
            description,
            parent_role_id,
        )
        .await
        .map_err(|e| match &e {
            DomainError::Internal(inner) if is_unique_violation(inner) => {
                DomainError::duplicate("Role with this code")
            }
            _ => e,
        })?;

        Ok(role)
    }

    async fn update_role(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
        role_name: &str,
        description: Option<&str>,
    ) -> Result<Role> {
        let role = IdentityRepo::update_role(&mut *db, role_id, role_name, description)
            .await
            .map_err(|e| match &e {
                DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("Role"),
                _ => e,
            })?;

        Ok(role)
    }

    async fn delete_role(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, role_id: i64) -> Result<()> {
        IdentityRepo::delete_role(&mut *db, role_id).await?;

        // Reload permission cache after role deletion
        let pool = self.pool.clone();
        self.cache.reload(&pool).await?;

        Ok(())
    }

    async fn list_roles(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<Vec<Role>> {
        IdentityRepo::list_roles(&mut *db).await
    }

    async fn assign_permissions(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<()> {
        IdentityRepo::assign_permissions(&mut *db, role_id, &permissions).await?;

        // Reload permission cache after permission change
        let pool = self.pool.clone();
        self.cache.reload(&pool).await?;

        Ok(())
    }

    async fn remove_permissions(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
        permissions: Vec<(String, String)>,
    ) -> Result<()> {
        IdentityRepo::remove_permissions(&mut *db, role_id, &permissions).await?;

        // Reload permission cache after permission change
        let pool = self.pool.clone();
        self.cache.reload(&pool).await?;

        Ok(())
    }

    async fn get_role_with_permissions(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
    ) -> Result<RoleWithPermissions> {
        let role = IdentityRepo::get_role_by_id(&mut *db, role_id)
            .await
            .map_err(|e| match &e {
                DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("Role"),
                _ => e,
            })?;

        let permissions =
            IdentityRepo::get_permissions_for_role(&mut *db, role_id).await?;

        Ok(RoleWithPermissions { role, permissions })
    }
}

fn is_unique_violation(err: &anyhow::Error) -> bool {
    err.downcast_ref::<sqlx::Error>()
        .map(|e| {
            if let sqlx::Error::Database(db_err) = e {
                db_err
                    .code()
                    .as_ref()
                    .map(|c| c == "23505")
                    .unwrap_or(false)
            } else {
                false
            }
        })
        .unwrap_or(false)
}

fn is_no_row(err: &anyhow::Error) -> bool {
    err.downcast_ref::<sqlx::Error>()
        .map(|e| matches!(e, sqlx::Error::RowNotFound))
        .unwrap_or(false)
}
