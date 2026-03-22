use anyhow::Result;
use sqlx::PgPool;

use crate::models::{Action, AuditLog, Permission, PermissionInfo, Resource};
use crate::repositories::Executor;

pub struct PermissionRepo;

impl PermissionRepo {
    pub async fn list_resources(pool: &PgPool) -> Result<Vec<Resource>> {
        let resources = sqlx::query_as!(
            Resource,
            r#"
            SELECT resource_id, resource_name, resource_code,
                   group_name, sort_order, description
            FROM resources
            ORDER BY sort_order
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(resources)
    }

    pub async fn list_actions(pool: &PgPool) -> Result<Vec<Action>> {
        let actions = sqlx::query_as!(
            Action,
            r#"
            SELECT action_code, action_name, sort_order, description
            FROM actions
            ORDER BY sort_order
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(actions)
    }

    pub async fn list_permissions(pool: &PgPool) -> Result<Vec<PermissionInfo>> {
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
            JOIN resources r ON p.resource_id = r.resource_id
            JOIN actions a ON p.action_code = a.action_code
            ORDER BY p.sort_order
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(permissions)
    }

    pub async fn get_user_permissions(
        pool: &PgPool,
        user_id: i64,
    ) -> Result<Vec<PermissionInfo>> {
        let permissions = sqlx::query_as!(
            PermissionInfo,
            r#"
            SELECT DISTINCT ON (p.permission_id)
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
            FROM user_roles ur
            JOIN role_permissions rp ON ur.role_id = rp.role_id
            JOIN permissions p ON rp.permission_id = p.permission_id
            JOIN resources r ON p.resource_id = r.resource_id
            JOIN actions a ON p.action_code = a.action_code
            WHERE ur.user_id = $1
            ORDER BY p.permission_id, p.sort_order
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        Ok(permissions)
    }

    pub async fn check_permission(
        pool: &PgPool,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool> {
        // 1. 检查是否超级管理员
        let is_super = sqlx::query_scalar!(
            "SELECT is_super_admin FROM users WHERE user_id = $1",
            user_id
        )
        .fetch_optional(pool)
        .await?;

        if is_super.unwrap_or(false) {
            return Ok(true);
        }

        // 2. 检查用户角色是否有此权限
        let has_permission = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM user_roles ur
                JOIN role_permissions rp ON ur.role_id = rp.role_id
                JOIN permissions p ON rp.permission_id = p.permission_id
                JOIN resources r ON p.resource_id = r.resource_id
                WHERE ur.user_id = $1
                  AND r.resource_code = $2
                  AND p.action_code = $3
            )
            "#,
            user_id,
            resource_code,
            action_code
        )
        .fetch_one(pool)
        .await?;

        Ok(has_permission.unwrap_or(false))
    }

    pub async fn insert_audit_log(
        executor: Executor<'_>,
        operator_id: i64,
        target_type: &str,
        target_id: i64,
        action: &str,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO permission_audit_logs
                (operator_id, target_type, target_id, action, old_value, new_value)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            operator_id,
            target_type,
            target_id,
            action,
            old_value,
            new_value
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn list_audit_logs(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as!(
            AuditLog,
            r#"
            SELECT
                l.log_id,
                l.operator_id,
                u.display_name as operator_name,
                l.target_type,
                l.target_id,
                l.action,
                l.old_value,
                l.new_value,
                l.created_at
            FROM permission_audit_logs l
            LEFT JOIN users u ON l.operator_id = u.user_id
            ORDER BY l.created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;

        Ok(logs)
    }

    pub async fn find_permission_by_id(
        pool: &PgPool,
        permission_id: i64,
    ) -> Result<Option<Permission>> {
        let permission = sqlx::query_as!(
            Permission,
            r#"
            SELECT permission_id, permission_name, resource_id,
                   action_code, sort_order, description
            FROM permissions
            WHERE permission_id = $1
            "#,
            permission_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(permission)
    }
}
