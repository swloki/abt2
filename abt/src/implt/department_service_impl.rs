use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use common::error::ServiceError;
use crate::models::*;
use crate::repositories::{DepartmentRepo, Executor};
use crate::service::DepartmentService;

pub struct DepartmentServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl DepartmentServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DepartmentService for DepartmentServiceImpl {
    async fn create(
        &self,
        req: CreateDepartmentRequest,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let department_id = DepartmentRepo::insert(executor, &req)
            .await
            .map_err(|e| map_duplicate_error(e, &req.department_code))?;
        Ok(department_id)
    }

    async fn update(
        &self,
        department_id: i64,
        req: UpdateDepartmentRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        let _old_dept = DepartmentRepo::find_by_id(self.pool.as_ref(), department_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Department".to_string(),
                id: department_id.to_string(),
            })?;
        DepartmentRepo::update(executor, department_id, &req).await?;
        Ok(())
    }

    async fn delete(
        &self,
        department_id: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        let old_dept = DepartmentRepo::find_by_id(self.pool.as_ref(), department_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Department".to_string(),
                id: department_id.to_string(),
            })?;
        if old_dept.is_default {
            return Err(ServiceError::BusinessValidation {
                message: "无法删除默认部门".to_string(),
            }.into());
        }
        DepartmentRepo::delete(executor, department_id).await?;
        Ok(())
    }

    async fn get(&self, department_id: i64) -> Result<Option<Department>> {
        let department = DepartmentRepo::find_by_id(self.pool.as_ref(), department_id).await?;
        Ok(department)
    }

    async fn list(&self, include_inactive: bool) -> Result<Vec<Department>> {
        let departments = DepartmentRepo::list_all(self.pool.as_ref(), include_inactive).await?;
        Ok(departments)
    }

    async fn get_user_departments(&self, user_id: i64) -> Result<Vec<Department>> {
        let departments = DepartmentRepo::get_user_departments(self.pool.as_ref(), user_id).await?;
        Ok(departments)
    }

    async fn assign_departments(
        &self,
        user_id: i64,
        department_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()> {
        DepartmentRepo::assign_departments(executor, user_id, &department_ids).await?;
        Ok(())
    }

    async fn remove_departments(
        &self,
        user_id: i64,
        department_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()> {
        DepartmentRepo::remove_departments(executor, user_id, &department_ids).await?;
        Ok(())
    }
}

fn map_duplicate_error(e: anyhow::Error, department_code: &str) -> anyhow::Error {
    if let Some(sqlx::Error::Database(db_err)) = e.downcast_ref::<sqlx::Error>() {
        if db_err.code().as_deref() == Some("23505") {
            return anyhow::Error::from(ServiceError::Conflict {
                resource: "Department".to_string(),
                message: format!("部门编码 '{}' 已存在", department_code),
            });
        }
    }
    e
}
