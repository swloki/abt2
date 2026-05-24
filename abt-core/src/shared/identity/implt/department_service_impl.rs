use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::department_service::DepartmentService;
use super::super::model::Department;
use super::super::repo::IdentityRepo;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

pub struct DepartmentServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl DepartmentServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DepartmentService for DepartmentServiceImpl {
    async fn create_department(
        &self,
        ctx: ServiceContext<'_>,
        name: &str,
        code: &str,
        description: Option<&str>,
    ) -> Result<Department, DomainError> {
        let dept = IdentityRepo::insert_department(&mut *ctx.executor, name, code, description)
            .await
            .map_err(|e| {
                if is_unique_violation(&e) {
                    DomainError::duplicate("Department with this code")
                } else {
                    DomainError::Internal(e.into())
                }
            })?;

        Ok(dept)
    }

    async fn update_department(
        &self,
        ctx: ServiceContext<'_>,
        dept_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<Department, DomainError> {
        let dept =
            IdentityRepo::update_department(&mut *ctx.executor, dept_id, name, description)
                .await
                .map_err(|e| {
                    if is_no_row(&e) {
                        DomainError::not_found("Department")
                    } else {
                        DomainError::Internal(e.into())
                    }
                })?;

        Ok(dept)
    }

    async fn delete_department(
        &self,
        ctx: ServiceContext<'_>,
        dept_id: i64,
    ) -> Result<(), DomainError> {
        IdentityRepo::deactivate_department(&mut *ctx.executor, dept_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    async fn list_departments(
        &self,
        ctx: ServiceContext<'_>,
    ) -> Result<Vec<Department>, DomainError> {
        IdentityRepo::list_departments(&mut *ctx.executor)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn assign_departments(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        dept_ids: Vec<i64>,
    ) -> Result<(), DomainError> {
        // Verify user exists
        IdentityRepo::get_user(&mut *ctx.executor, user_id)
            .await
            .map_err(|e| {
                if is_no_row(&e) {
                    DomainError::not_found("User")
                } else {
                    DomainError::Internal(e.into())
                }
            })?;

        // Get current departments and merge
        let current = IdentityRepo::get_user_department_ids(&mut *ctx.executor, user_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let mut merged: Vec<i64> = current;
        for id in &dept_ids {
            if !merged.contains(id) {
                merged.push(*id);
            }
        }

        IdentityRepo::replace_user_departments(&mut *ctx.executor, user_id, &merged)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn remove_departments(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        dept_ids: Vec<i64>,
    ) -> Result<(), DomainError> {
        IdentityRepo::remove_user_departments(&mut *ctx.executor, user_id, &dept_ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

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
