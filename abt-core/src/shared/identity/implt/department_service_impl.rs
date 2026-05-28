use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::department_service::DepartmentService;
use super::super::model::Department;
use super::super::repo::IdentityRepo;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

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
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        name: &str,
        code: &str,
        description: Option<&str>,
    ) -> Result<Department> {
        let dept = IdentityRepo::insert_department(&mut *db, name, code, description)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_unique_violation(inner) => DomainError::duplicate("Department with this code"), _ => e })?;

        Ok(dept)
    }

    async fn update_department(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        dept_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<Department> {
        let dept =
            IdentityRepo::update_department(&mut *db, dept_id, name, description)
                .await
                .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("Department"), _ => e })?;

        Ok(dept)
    }

    async fn delete_department(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        dept_id: i64,
    ) -> Result<()> {
        IdentityRepo::deactivate_department(&mut *db, dept_id)
            .await
            ?;
        Ok(())
    }

    async fn get_department(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        dept_id: i64,
    ) -> Result<Department> {
        IdentityRepo::get_department(&mut *db, dept_id)
            .await
            .map_err(|e| match &e {
                DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("Department"),
                _ => e,
            })
    }

    async fn list_departments(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
    ) -> Result<Vec<Department>> {
        IdentityRepo::list_departments(&mut *db).await
    }

    async fn assign_departments(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        dept_ids: Vec<i64>,
    ) -> Result<()> {
        // Verify user exists
        IdentityRepo::get_user(&mut *db, user_id)
            .await
            .map_err(|e| match &e { DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"), _ => e })?;

        // Get current departments and merge
        let current = IdentityRepo::get_user_department_ids(&mut *db, user_id)
            .await
            ?;

        let mut merged: Vec<i64> = current;
        for id in &dept_ids {
            if !merged.contains(id) {
                merged.push(*id);
            }
        }

        IdentityRepo::replace_user_departments(&mut *db, user_id, &merged)
            .await
            ?;

        Ok(())
    }

    async fn remove_departments(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
        dept_ids: Vec<i64>,
    ) -> Result<()> {
        IdentityRepo::remove_user_departments(&mut *db, user_id, &dept_ids)
            .await
            ?;

        Ok(())
    }

    async fn get_user_departments(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        user_id: i64,
    ) -> Result<Vec<Department>> {
        IdentityRepo::get_user_departments(&mut *db, user_id).await
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
