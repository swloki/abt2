use async_trait::async_trait;

use super::model::Department;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

#[async_trait]
pub trait DepartmentService: Send + Sync {
    async fn create_department(
        &self,
        ctx: ServiceContext<'_>,
        name: &str,
        code: &str,
        description: Option<&str>,
    ) -> Result<Department>;

    async fn update_department(
        &self,
        ctx: ServiceContext<'_>,
        dept_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<Department>;

    async fn delete_department(
        &self,
        ctx: ServiceContext<'_>,
        dept_id: i64,
    ) -> Result<()>;

    async fn get_department(
        &self,
        ctx: ServiceContext<'_>,
        dept_id: i64,
    ) -> Result<Department>;

    async fn list_departments(
        &self,
        ctx: ServiceContext<'_>,
    ) -> Result<Vec<Department>>;

    async fn assign_departments(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        dept_ids: Vec<i64>,
    ) -> Result<()>;

    async fn remove_departments(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
        dept_ids: Vec<i64>,
    ) -> Result<()>;

    async fn get_user_departments(
        &self,
        ctx: ServiceContext<'_>,
        user_id: i64,
    ) -> Result<Vec<Department>>;
}
