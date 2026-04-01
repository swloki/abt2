use anyhow::Result;
use sqlx::PgPool;

use crate::models::AuditLog;
use crate::repositories::Executor;

pub struct PermissionRepo;

impl PermissionRepo {
    /// Check if user is super admin
    pub async fn is_super_admin(pool: &PgPool, user_id: i64) -> Result<bool> {
        let is_super = sqlx::query_scalar!(
            "SELECT is_super_admin FROM users WHERE user_id = $1",
            user_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(is_super.unwrap_or(false))
    }

    /// Get user's permission codes (resource_code:action_code) from role_permissions
    pub async fn get_user_permission_codes(pool: &PgPool, user_id: i64) -> Result<Vec<String>> {
        let codes: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT CONCAT(rp.resource_code, ':', rp.action_code) as "permission"
            FROM user_roles ur
            JOIN role_permissions rp ON ur.role_id = rp.role_id
            WHERE ur.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(codes.into_iter().map(|(p,)| p).collect())
    }

    /// Check if user has a specific permission
    pub async fn check_permission(
        pool: &PgPool,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool> {
        if Self::is_super_admin(pool, user_id).await? {
            return Ok(true);
        }

        let has_permission = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM user_roles ur
                JOIN role_permissions rp ON ur.role_id = rp.role_id
                WHERE ur.user_id = $1
                  AND rp.resource_code = $2
                  AND rp.action_code = $3
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
        operator_id: Option<i64>,
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
}
