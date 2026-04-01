use anyhow::Result;
use async_trait::async_trait;

use crate::models::{CreateDepartmentRequest, Department, UpdateDepartmentRequest};
use crate::repositories::Executor;

#[async_trait]
pub trait DepartmentService: Send + Sync {
    async fn create(
        &self,
        operator_id: Option<i64>,
        req: CreateDepartmentRequest,
        executor: Executor<'_>,
    ) -> Result<i64>;

    async fn update(
        &self,
        operator_id: Option<i64>,
        department_id: i64,
        req: UpdateDepartmentRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn delete(
        &self,
        operator_id: Option<i64>,
        department_id: i64,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn get(&self, department_id: i64) -> Result<Option<Department>>;

    async fn list(&self, include_inactive: bool) -> Result<Vec<Department>>;

    async fn get_user_departments(&self, user_id: i64) -> Result<Vec<Department>>;

    async fn assign_departments(
        &self,
        operator_id: Option<i64>,
        user_id: i64,
        department_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn remove_departments(
        &self,
        operator_id: Option<i64>,
        user_id: i64,
        department_ids: Vec<i64>,
        executor: Executor<'_>,
    ) -> Result<()>;
}
