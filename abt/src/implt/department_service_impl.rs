use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

use crate::models::*;
use crate::repositories::{DepartmentRepo, DepartmentResourceAccessRepo, Executor, UserDepartmentRoleRepo};
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
        let department_id = DepartmentRepo::insert(executor, &req).await?;
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
            .ok_or_else(|| anyhow!("Department not found"))?;
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
            .ok_or_else(|| anyhow!("Department not found"))?;
        if old_dept.is_default {
            return Err(anyhow!("Cannot delete the default department"));
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

    async fn set_department_resources(
        &self,
        department_id: i64,
        resource_codes: Vec<String>,
        executor: Executor<'_>,
    ) -> Result<Vec<String>> {
        // Validate department exists and is active (R11c)
        let dept = DepartmentRepo::find_by_id(self.pool.as_ref(), department_id)
            .await?
            .ok_or_else(|| anyhow!("Department not found"))?;
        if !dept.is_active {
            return Err(anyhow!("Department is inactive"));
        }

        // Validate and filter resource codes (R11b)
        let business_codes = DepartmentResourceAccessRepo::validate_resource_codes(&resource_codes)?;

        // Full overwrite (R11)
        DepartmentResourceAccessRepo::set_department_resources(executor, department_id, &business_codes).await?;

        Ok(business_codes)
    }

    async fn get_department_resources(
        &self,
        department_id: i64,
    ) -> Result<Vec<String>> {
        let codes = DepartmentResourceAccessRepo::get_department_resources(self.pool.as_ref(), department_id).await?;
        Ok(codes)
    }

    async fn assign_user_dept_roles(
        &self,
        user_id: i64,
        assignments: Vec<DeptRole>,
        executor: Executor<'_>,
    ) -> Result<()> {
        UserDepartmentRoleRepo::assign(executor, user_id, &assignments).await
    }

    async fn remove_user_dept_roles(
        &self,
        user_id: i64,
        assignments: Vec<DeptRole>,
        executor: Executor<'_>,
    ) -> Result<()> {
        UserDepartmentRoleRepo::remove(executor, user_id, &assignments).await
    }

    async fn get_user_dept_roles(
        &self,
        user_id: i64,
    ) -> Result<Vec<DeptRoleDetail>> {
        UserDepartmentRoleRepo::get_user_dept_role_details(self.pool.as_ref(), user_id).await
    }
}
