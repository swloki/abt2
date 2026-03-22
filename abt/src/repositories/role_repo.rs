use anyhow::Result;
use sqlx::{PgPool, QueryBuilder, Postgres};

use crate::models::{CreateRoleRequest, PermissionInfo, Role, UpdateRoleRequest};
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
                   description, created_at, updated_at
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
                   description, created_at, updated_at
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
                   description, created_at, updated_at
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
                   description, created_at, updated_at
            FROM roles
            ORDER BY role_id
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(roles)
    }

    pub async fn get_role_permissions(
        pool: &PgPool,
        role_id: i64,
    ) -> Result<Vec<PermissionInfo>> {
        let permissions = sqlx::query_as!(
            PermissionInfo,
            r#"
            SELECT
                p.permission_id,
                p.permission_name,
                r.resource_id as "resource_id!",
                r.resource_name as "resource_name!",
                r.resource_code as "resource_code!",
                r.group_name as "group_name!",
                r.sort_order as "resource_sort_order!",
                r.description as "resource_description!",
                p.action_code,
                a.action_name
            FROM permissions p
            JOIN role_permissions rp ON p.permission_id = rp.permission_id
            JOIN resources r ON p.resource_id = r.resource_id
            JOIN actions a ON p.action_code = a.action_code
            WHERE rp.role_id = $1
            ORDER BY p.sort_order
            "#,
            role_id
        )
        .fetch_all(pool)
        .await?;

        Ok(permissions)
    }

    pub async fn assign_permissions(
        executor: Executor<'_>,
        role_id: i64,
        permission_ids: &[i64],
    ) -> Result<()> {
        if permission_ids.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO role_permissions (role_id, permission_id) "
        );
        builder.push_values(permission_ids.iter(), |mut b, pid| {
            b.push_bind(role_id).push_bind(*pid);
        });
        builder.push(" ON CONFLICT DO NOTHING");

        builder.build().execute(executor).await?;

        Ok(())
    }

    pub async fn remove_permissions(
        executor: Executor<'_>,
        role_id: i64,
        permission_ids: &[i64],
    ) -> Result<()> {
        if permission_ids.is_empty() {
            return Ok(());
        }

        sqlx::query!(
            "DELETE FROM role_permissions WHERE role_id = $1 AND permission_id = ANY($2)",
            role_id,
            permission_ids
        )
        .execute(executor)
        .await?;

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
