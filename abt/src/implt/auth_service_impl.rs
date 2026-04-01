use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

use crate::models::{Claims, ResourceActionDef};
use crate::repositories::AuthRepo;
use crate::service::AuthService;

const SECONDS_PER_HOUR: u64 = 3600;

pub struct AuthServiceImpl {
    pool: Arc<sqlx::PgPool>,
    jwt_secret: String,
    jwt_expiration_hours: u64,
    resource_actions: Vec<ResourceActionDef>,
}

impl AuthServiceImpl {
    pub fn new(
        pool: Arc<sqlx::PgPool>,
        jwt_secret: String,
        jwt_expiration_hours: u64,
        resource_actions: Vec<ResourceActionDef>,
    ) -> Self {
        Self {
            pool,
            jwt_secret,
            jwt_expiration_hours,
            resource_actions,
        }
    }

    /// 签发 JWT
    fn sign_jwt(&self, claims: &Claims) -> Result<String> {
        let token = encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        Ok(token)
    }

    /// 验证 JWT 并返回 Claims
    fn verify_jwt(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )?;
        Ok(token_data.claims)
    }

    /// 构建 Claims
    fn build_claims(
        user_id: i64,
        username: String,
        display_name: String,
        is_super_admin: bool,
        permissions: Vec<String>,
        now: u64,
        expiration_hours: u64,
    ) -> Claims {
        Claims {
            sub: user_id,
            username,
            display_name,
            is_super_admin,
            permissions,
            iat: now,
            exp: now + expiration_hours * SECONDS_PER_HOUR,
        }
    }
}

#[async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(&self, username: &str, password: &str) -> Result<(String, i64, Claims)> {
        // 1. 查找用户
        let user = AuthRepo::find_user_by_username(self.pool.as_ref(), username)
            .await?
            .ok_or_else(|| anyhow!("Invalid username or password"))?;

        // 2. 检查是否启用
        if !user.is_active {
            return Err(anyhow!("User account is disabled"));
        }

        // 3. 验证密码 (argon2)
        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| anyhow!("Invalid password hash format"))?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| anyhow!("Invalid username or password"))?;

        // 4. 获取权限
        let permissions = if user.is_super_admin {
            vec![] // super_admin 不需要 permissions 列表
        } else {
            AuthRepo::get_user_permission_codes(self.pool.as_ref(), user.user_id).await?
        };

        // 5. 签发 JWT
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let display_name = user.display_name.clone().unwrap_or_default();
        let claims = Self::build_claims(
            user.user_id,
            user.username.clone(),
            display_name,
            user.is_super_admin,
            permissions,
            now,
            self.jwt_expiration_hours,
        );

        let expires_at = claims.exp as i64;
        let token = self.sign_jwt(&claims)?;

        Ok((token, expires_at, claims))
    }

    async fn refresh_token(&self, token: &str) -> Result<(String, i64, Claims)> {
        // 验证旧 token
        let old_claims = self.verify_jwt(token)?;

        // 确认用户仍然存在且启用
        let user = AuthRepo::find_user_by_id(self.pool.as_ref(), old_claims.sub)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        if !user.is_active {
            return Err(anyhow!("User account is disabled"));
        }

        // 获取最新权限
        let permissions = if user.is_super_admin {
            vec![]
        } else {
            AuthRepo::get_user_permission_codes(self.pool.as_ref(), user.user_id).await?
        };

        // 签发新 token
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let display_name = user.display_name.clone().unwrap_or_default();
        let claims = Self::build_claims(
            user.user_id,
            user.username.clone(),
            display_name,
            user.is_super_admin,
            permissions,
            now,
            self.jwt_expiration_hours,
        );

        let expires_at = claims.exp as i64;
        let new_token = self.sign_jwt(&claims)?;

        Ok((new_token, expires_at, claims))
    }

    async fn get_user_claims(&self, user_id: i64) -> Result<Claims> {
        let user = AuthRepo::find_user_by_id(self.pool.as_ref(), user_id)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        let permissions = if user.is_super_admin {
            vec![]
        } else {
            AuthRepo::get_user_permission_codes(self.pool.as_ref(), user.user_id).await?
        };

        let display_name = user.display_name.clone().unwrap_or_default();
        Ok(Claims {
            sub: user.user_id,
            username: user.username,
            display_name,
            is_super_admin: user.is_super_admin,
            permissions,
            exp: 0,
            iat: 0,
        })
    }

    fn list_resources(&self) -> Vec<ResourceActionDef> {
        self.resource_actions.clone()
    }
}
