use anyhow::Result;
use sqlx::PgPool;

use crate::models::{DeptRole, DeptRoleDetail};
use crate::repositories::Executor;

pub struct UserDepartmentRoleRepo;

impl UserDepartmentRoleRepo {
    /// Assign roles to a user in specific departments (merge semantics)
    pub async fn assign(
        executor: Executor<'_>,
        user_id: i64,
        assignments: &[DeptRole],
    ) -> Result<()> {
        for dept_role in assignments {
            sqlx::query!(
                r#"
                INSERT INTO user_department_roles (user_id, department_id, role_id)
                VALUES ($1, $2, $3)
                ON CONFLICT (user_id, department_id, role_id) DO NOTHING
                "#,
                user_id,
                dept_role.department_id,
                dept_role.role_id,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// Remove specific role assignments for a user
    pub async fn remove(
        executor: Executor<'_>,
        user_id: i64,
        assignments: &[DeptRole],
    ) -> Result<()> {
        for dept_role in assignments {
            sqlx::query!(
                r#"
                DELETE FROM user_department_roles
                WHERE user_id = $1 AND department_id = $2 AND role_id = $3
                "#,
                user_id,
                dept_role.department_id,
                dept_role.role_id,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// Get all department-role assignments for a user
    pub async fn get_user_dept_roles(
        pool: &PgPool,
        user_id: i64,
    ) -> Result<Vec<DeptRole>> {
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            r#"
            SELECT department_id, role_id
            FROM user_department_roles
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(department_id, role_id)| DeptRole {
                department_id,
                role_id,
            })
            .collect())
    }

    /// Get all department-role assignments for a user (with names, for API)
    pub async fn get_user_dept_role_details(
        pool: &PgPool,
        user_id: i64,
    ) -> Result<Vec<DeptRoleDetail>> {
        let rows = sqlx::query_as!(
            DeptRoleDetail,
            r#"
            SELECT
                udr.department_id,
                d.department_name,
                udr.role_id,
                r.role_name
            FROM user_department_roles udr
            JOIN departments d ON d.department_id = udr.department_id
            JOIN roles r ON r.role_id = udr.role_id
            WHERE udr.user_id = $1
            ORDER BY udr.department_id, udr.role_id
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Get role IDs for a user in a specific department
    pub async fn get_user_dept_role_ids(
        pool: &PgPool,
        user_id: i64,
        department_id: i64,
    ) -> Result<Vec<i64>> {
        let ids: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT role_id FROM user_department_roles
            WHERE user_id = $1 AND department_id = $2
            "#,
        )
        .bind(user_id)
        .bind(department_id)
        .fetch_all(pool)
        .await?;

        Ok(ids.into_iter().map(|(id,)| id).collect())
    }

    /// Remove all role assignments for a user in a department
    pub async fn remove_all_for_dept(
        executor: Executor<'_>,
        user_id: i64,
        department_id: i64,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM user_department_roles WHERE user_id = $1 AND department_id = $2",
            user_id,
            department_id,
        )
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
}
