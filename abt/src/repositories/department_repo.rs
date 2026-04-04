use anyhow::Result;
use sqlx::PgPool;

use crate::models::{CreateDepartmentRequest, Department, UpdateDepartmentRequest};
use crate::repositories::Executor;

pub struct DepartmentRepo;

impl DepartmentRepo {
    pub async fn insert(
        executor: Executor<'_>,
        req: &CreateDepartmentRequest,
    ) -> Result<i64> {
        let department_id = sqlx::query_scalar!(
            r#"
            INSERT INTO departments (department_name, department_code, description)
            VALUES ($1, $2, $3)
            RETURNING department_id
            "#,
            req.department_name,
            req.department_code,
            req.description
        )
        .fetch_one(executor)
        .await?;

        Ok(department_id)
    }

    pub async fn update(
        executor: Executor<'_>,
        department_id: i64,
        req: &UpdateDepartmentRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE departments SET
                department_name = COALESCE($2, department_name),
                description = COALESCE($3, description),
                is_active = COALESCE($4, is_active),
                updated_at = NOW()
            WHERE department_id = $1
            "#,
            department_id,
            req.department_name,
            req.description,
            req.is_active
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn delete(executor: Executor<'_>, department_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM departments WHERE department_id = $1", department_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, department_id: i64) -> Result<Option<Department>> {
        let department = sqlx::query_as!(
            Department,
            r#"
            SELECT department_id, department_name, department_code,
                   description, is_active, is_default, created_at, updated_at
            FROM departments
            WHERE department_id = $1
            "#,
            department_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(department)
    }

    pub async fn find_by_code(pool: &PgPool, department_code: &str) -> Result<Option<Department>> {
        let department = sqlx::query_as!(
            Department,
            r#"
            SELECT department_id, department_name, department_code,
                   description, is_active, is_default, created_at, updated_at
            FROM departments
            WHERE department_code = $1
            "#,
            department_code
        )
        .fetch_optional(pool)
        .await?;

        Ok(department)
    }

    pub async fn list_all(pool: &PgPool, include_inactive: bool) -> Result<Vec<Department>> {
        let departments = if include_inactive {
            sqlx::query_as!(
                Department,
                r#"
                SELECT department_id, department_name, department_code,
                       description, is_active, is_default, created_at, updated_at
                FROM departments
                ORDER BY department_id
                "#
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as!(
                Department,
                r#"
                SELECT department_id, department_name, department_code,
                       description, is_active, is_default, created_at, updated_at
                FROM departments
                WHERE is_active = true
                ORDER BY department_id
                "#
            )
            .fetch_all(pool)
            .await?
        };

        Ok(departments)
    }

    pub async fn get_user_departments(pool: &PgPool, user_id: i64) -> Result<Vec<Department>> {
        let departments = sqlx::query_as!(
            Department,
            r#"
            SELECT d.department_id, d.department_name, d.department_code,
                   d.description, d.is_active, d.is_default, d.created_at, d.updated_at
            FROM departments d
            JOIN user_departments ud ON d.department_id = ud.department_id
            WHERE ud.user_id = $1
            ORDER BY d.department_id
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        Ok(departments)
    }

    pub async fn assign_departments(
        executor: Executor<'_>,
        user_id: i64,
        department_ids: &[i64],
    ) -> Result<()> {
        if department_ids.is_empty() {
            return Ok(());
        }

        // Batch insert using UNNEST to avoid N+1 queries
        sqlx::query!(
            r#"
            INSERT INTO user_departments (user_id, department_id)
            SELECT $1, unnest($2::bigint[])
            ON CONFLICT (user_id, department_id) DO NOTHING
            "#,
            user_id,
            department_ids
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn remove_departments(
        executor: Executor<'_>,
        user_id: i64,
        department_ids: &[i64],
    ) -> Result<()> {
        if department_ids.is_empty() {
            return Ok(());
        }

        sqlx::query!(
            r#"
            DELETE FROM user_departments
            WHERE user_id = $1 AND department_id = ANY($2)
            "#,
            user_id,
            department_ids
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn get_user_department_ids(pool: &PgPool, user_id: i64) -> Result<Vec<i64>> {
        let ids = sqlx::query_scalar!(
            r#"
            SELECT department_id
            FROM user_departments
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        Ok(ids)
    }
}
