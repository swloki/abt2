use anyhow::Result;
use sqlx::PgPool;

use crate::models::User;
use crate::repositories::UserRepo;

pub struct AuthRepo;

impl AuthRepo {
    /// 根据用户名查找用户
    pub async fn find_user_by_username(pool: &PgPool, username: &str) -> Result<Option<User>> {
        UserRepo::find_by_username(pool, username).await
    }

    /// 根据用户 ID 查找用户
    pub async fn find_user_by_id(pool: &PgPool, user_id: i64) -> Result<Option<User>> {
        UserRepo::find_by_id(pool, user_id).await
    }

    /// Get user's global role IDs from user_roles table
    pub async fn get_user_role_ids(pool: &PgPool, user_id: i64) -> Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT role_id
            FROM user_roles
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}
