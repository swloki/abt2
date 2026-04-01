use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

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

    async fn log_audit(
        &self,
        executor: Executor<'_>,
        operator_id: Option<i64>,
        target_type: &str,
        target_id: i64,
        action: &str,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    ) -> Result<()> {
        // 部门模块暂时不记录审计日志，与角色管理保持一致
        let _ = (executor, operator_id, target_type, target_id, action, old_value, new_value);
        Ok(())
    }
}

#[async_trait]
impl DepartmentService for DepartmentServiceImpl {
    async fn create(
        &self,
        operator_id: Option<i64>,
        req: CreateDepartmentRequest,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let department_id = DepartmentRepo::insert(executor, &req).await?;
        self.log_audit(
            executor,
            operator_id,
            "department",
            department_id,
            "create",
            None,
            Some(serde_json::to_value(&req)?),
        )
        .await?;
        Ok(department_id)
    }

    async fn update(
        &self,
        operator_id: Option<i64>,
        department_id: i64,
        req: UpdateDepartmentRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        let old_dept = DepartmentRepo::find_by_id(self.pool.as_ref(), department_id)
            .await?
            .ok_or_else(|| anyhow!("Department not found"))?;
        DepartmentRepo::update(executor, department_id, &req).await?;
        self.log_audit(
            executor,
            operator_id,
            "department",
            department_id,
            "update",
            Some(serde_json::to_value(&old_dept)?),
            Some(serde_json::to_value(&req)?),
        )
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        operator_id: Option<i64>,
        department_id: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        let old_dept = DepartmentRepo::find_by_id(self.pool.as_ref(), department_id)
            .await?
            .ok_or_else(|| anyhow!("Department not found"))?;
        DepartmentRepo::delete(executor, department_id).await?;
        self.log_audit(
            executor,
            operator_id,
            "department",
            department_id,
            "delete",
            Some(serde_json::to_value(&old_dept)?),
            None,
        )
        .await?;
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
        operator_id: Option<i64>,
        user_id: i64,
        department_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()> {
        DepartmentRepo::assign_departments(executor, user_id, &department_ids).await?;
        self.log_audit(
            executor,
            operator_id,
            "user",
            user_id,
            "assign_departments",
            None,
            Some(serde_json::to_value(&department_ids)?),
        )
        .await?;
        Ok(())
    }

    async fn remove_departments(
        &self,
        operator_id: Option<i64>,
        user_id: i64,
        department_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()> {
        DepartmentRepo::remove_departments(executor, user_id, &department_ids).await?;
        self.log_audit(
            executor,
            operator_id,
            "user",
            user_id,
            "remove_departments",
            Some(serde_json::to_value(&department_ids)?),
            None,
        )
        .await?;
        Ok(())
    }
}
