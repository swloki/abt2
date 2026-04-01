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

    /// 获取用户的所有权限 (resource_code:action_code 列表)
    /// 从新的 role_permissions 表直接读取
    pub async fn get_user_permission_codes(pool: &PgPool, user_id: i64) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
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

        Ok(rows.into_iter().map(|(p,)| p).collect())
    }
}
