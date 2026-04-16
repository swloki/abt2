use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;

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

    /// Get user's department-role mappings as a nested map:
    /// { department_id_string => [role_id, ...] }
    pub async fn get_user_dept_roles(pool: &PgPool, user_id: i64) -> Result<HashMap<String, Vec<i64>>> {
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

        let mut map: HashMap<String, Vec<i64>> = HashMap::new();
        for (dept_id, role_id) in rows {
            map.entry(dept_id.to_string())
                .or_default()
                .push(role_id);
        }
        Ok(map)
    }
}
