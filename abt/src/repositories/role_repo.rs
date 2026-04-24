use anyhow::Result;
use sqlx::{PgPool, QueryBuilder, Postgres};

use crate::models::{CreateRoleRequest, Role, UpdateRoleRequest};
use crate::repositories::Executor;

pub struct RoleRepo;

impl RoleRepo {
    pub async fn insert(
        executor: Executor<'_>,
        req: &CreateRoleRequest,
    ) -> Result<i64> {
        let role_id = sqlx::query_scalar!(
            r#"
            INSERT INTO roles (role_name, role_code, description)
            VALUES ($1, $2, $3)
            RETURNING role_id
            "#,
            req.role_name,
            req.role_code,
            req.description
        )
        .fetch_one(executor)
        .await?;

        Ok(role_id)
    }

    pub async fn update(
        executor: Executor<'_>,
        role_id: i64,
        req: &UpdateRoleRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE roles SET
                role_name = COALESCE($2, role_name),
                description = COALESCE($3, description),
                updated_at = NOW()
            WHERE role_id = $1
            "#,
            role_id,
            req.role_name,
            req.description
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn delete(executor: Executor<'_>, role_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM roles WHERE role_id = $1", role_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, role_id: i64) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT role_id, role_name, role_code, is_system_role,
                   parent_role_id, description, created_at, updated_at
            FROM roles
            WHERE role_id = $1
            "#,
            role_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn find_by_id_with_executor(executor: Executor<'_>, role_id: i64) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT role_id, role_name, role_code, is_system_role,
                   parent_role_id, description, created_at, updated_at
            FROM roles
            WHERE role_id = $1
            "#,
            role_id
        )
        .fetch_optional(executor)
        .await?;

        Ok(role)
    }

    pub async fn find_by_code(pool: &PgPool, role_code: &str) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT role_id, role_name, role_code, is_system_role,
                   parent_role_id, description, created_at, updated_at
            FROM roles
            WHERE role_code = $1
            "#,
            role_code
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn list_all(pool: &PgPool) -> Result<Vec<Role>> {
        let roles = sqlx::query_as!(
            Role,
            r#"
            SELECT role_id, role_name, role_code, is_system_role,
                   parent_role_id, description, created_at, updated_at
            FROM roles
            ORDER BY role_id
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(roles)
    }

    /// Get role permissions as resource_code:action_code pairs
    pub async fn get_role_permission_codes(
        pool: &PgPool,
        role_id: i64,
    ) -> Result<Vec<String>> {
        let codes: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT CONCAT(rp.resource_code, ':', rp.action_code) as "code"
            FROM role_permissions rp
            WHERE rp.role_id = $1
            "#,
        )
        .bind(role_id)
        .fetch_all(pool)
        .await?;

        Ok(codes.into_iter().map(|(c,)| c).collect())
    }

    /// Assign permissions — full replacement semantics.
    /// Deletes all existing permissions for the role, then inserts the new set.
    pub async fn assign_permissions(
        executor: Executor<'_>,
        role_id: i64,
        resource_actions: &[(String, String)],
    ) -> Result<()> {
        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(executor.as_mut())
            .await?;

        if resource_actions.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO role_permissions (role_id, resource_code, action_code) "
        );
        builder.push_values(resource_actions.iter(), |mut b, (resource, action)| {
            b.push_bind(role_id).push_bind(resource.clone()).push_bind(action.clone());
        });

        builder.build().execute(executor).await?;

        Ok(())
    }

    /// Remove specific permissions from a role
    pub async fn remove_permissions(
        executor: Executor<'_>,
        role_id: i64,
        resource_actions: &[(String, String)],
    ) -> Result<()> {
        if resource_actions.is_empty() {
            return Ok(());
        }

        for (resource_code, action_code) in resource_actions {
            sqlx::query!(
                "DELETE FROM role_permissions WHERE role_id = $1 AND resource_code = $2 AND action_code = $3",
                role_id,
                resource_code,
                action_code
            )
            .execute(&mut *executor)
            .await?;
        }

        Ok(())
    }

    pub async fn is_system_role(pool: &PgPool, role_id: i64) -> Result<bool> {
        let is_system = sqlx::query_scalar!(
            "SELECT is_system_role FROM roles WHERE role_id = $1",
            role_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(is_system.unwrap_or(false))
    }
}
