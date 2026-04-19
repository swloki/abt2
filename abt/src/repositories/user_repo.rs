use anyhow::Result;
use sqlx::{PgPool, QueryBuilder, Postgres};

use crate::models::{CreateUserRequest, RoleInfo, UpdateUserRequest, User};
use crate::repositories::Executor;

pub struct UserRepo;

impl UserRepo {
    pub async fn insert(
        executor: Executor<'_>,
        req: &CreateUserRequest,
        password_hash: &str,
    ) -> Result<i64> {
        let user_id = sqlx::query_scalar!(
            r#"
            INSERT INTO users (username, password_hash, display_name, is_super_admin)
            VALUES ($1, $2, $3, $4)
            RETURNING user_id
            "#,
            req.username,
            password_hash,
            req.display_name,
            req.is_super_admin
        )
        .fetch_one(executor)
        .await?;

        Ok(user_id)
    }

    pub async fn update(
        executor: Executor<'_>,
        user_id: i64,
        req: &UpdateUserRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE users SET
                display_name = COALESCE($2, display_name),
                is_active = COALESCE($3, is_active),
                updated_at = NOW()
            WHERE user_id = $1
            "#,
            user_id,
            req.display_name,
            req.is_active
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn delete(executor: Executor<'_>, user_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM users WHERE user_id = $1", user_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, user_id: i64) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_id_with_executor(executor: Executor<'_>, user_id: i64) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_optional(executor)
        .await?;

        Ok(user)
    }

    pub async fn find_by_username(pool: &PgPool, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn list_all(pool: &PgPool) -> Result<Vec<User>> {
        let users = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            ORDER BY user_id
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(users)
    }

    pub async fn find_by_ids(pool: &PgPool, user_ids: &[i64]) -> Result<Vec<User>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let users = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            WHERE user_id = ANY($1)
            ORDER BY user_id
            "#,
            user_ids
        )
        .fetch_all(pool)
        .await?;

        Ok(users)
    }

    pub async fn get_user_roles(pool: &PgPool, user_id: i64) -> Result<Vec<RoleInfo>> {
        let roles = sqlx::query_as!(
            RoleInfo,
            r#"
            SELECT r.role_id, r.role_name, r.role_code
            FROM roles r
            JOIN user_roles ur ON r.role_id = ur.role_id
            WHERE ur.user_id = $1
            ORDER BY r.role_id
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        Ok(roles)
    }

    pub async fn assign_roles(
        executor: Executor<'_>,
        user_id: i64,
        role_ids: &[i64],
    ) -> Result<()> {
        if role_ids.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO user_roles (user_id, role_id) "
        );
        builder.push_values(role_ids.iter(), |mut b, role_id| {
            b.push_bind(user_id).push_bind(*role_id);
        });
        builder.push(" ON CONFLICT DO NOTHING");

        builder.build().execute(executor).await?;

        Ok(())
    }

    pub async fn remove_roles(
        executor: Executor<'_>,
        user_id: i64,
        role_ids: &[i64],
    ) -> Result<()> {
        if role_ids.is_empty() {
            return Ok(());
        }

        sqlx::query!(
            "DELETE FROM user_roles WHERE user_id = $1 AND role_id = ANY($2)",
            user_id,
            role_ids
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn batch_assign_roles(
        executor: Executor<'_>,
        user_ids: &[i64],
        role_ids: &[i64],
    ) -> Result<()> {
        if user_ids.is_empty() || role_ids.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO user_roles (user_id, role_id) "
        );
        builder.push_values(user_ids.iter().flat_map(|uid| role_ids.iter().map(move |rid| (*uid, *rid))), |mut b, (uid, rid)| {
            b.push_bind(uid).push_bind(rid);
        });
        builder.push(" ON CONFLICT DO NOTHING");

        builder.build().execute(executor).await?;

        Ok(())
    }
}
