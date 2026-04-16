use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use async_trait::async_trait;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

use crate::models::{Claims, ResourceActionDef};
use crate::repositories::{AuthRepo, DepartmentResourceAccessRepo};
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
        system_role: String,
        dept_roles: HashMap<String, Vec<i64>>,
        current_department_id: Option<i64>,
        now: u64,
        expiration_hours: u64,
    ) -> Claims {
        Claims {
            sub: user_id,
            username,
            display_name,
            system_role,
            dept_roles,
            current_department_id,
            iat: now,
            exp: now + expiration_hours * SECONDS_PER_HOUR,
        }
    }

    /// Resolve default department: if only one dept, auto-select; else None (frontend chooses).
    async fn resolve_default_department(
        &self,
        dept_roles: &HashMap<String, Vec<i64>>,
    ) -> Result<Option<i64>> {
        let dept_ids: Vec<i64> = dept_roles.keys()
            .filter_map(|k| k.parse::<i64>().ok())
            .collect();

        if dept_ids.len() == 1 {
            return Ok(Some(dept_ids[0]));
        }

        // Multiple or zero departments — use default department if no assignments
        if dept_ids.is_empty() {
            let default_id = DepartmentResourceAccessRepo::get_default_department_id(
                self.pool.as_ref(),
            ).await?;
            return Ok(default_id);
        }

        // Multiple departments — frontend will choose
        Ok(None)
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

        // 4. Determine system_role
        let system_role = if user.is_super_admin {
            "super_admin".to_string()
        } else {
            "user".to_string()
        };

        // 5. Get dept_roles from user_department_roles
        let dept_roles = AuthRepo::get_user_dept_roles(self.pool.as_ref(), user.user_id).await?;

        // 6. Determine current_department_id
        let current_department_id = self.resolve_default_department(&dept_roles).await?;

        // 7. Build and sign JWT
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let display_name = user.display_name.clone().unwrap_or_default();
        let claims = Self::build_claims(
            user.user_id,
            user.username.clone(),
            display_name,
            system_role,
            dept_roles,
            current_department_id,
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

        // Determine system_role
        let system_role = if user.is_super_admin {
            "super_admin".to_string()
        } else {
            "user".to_string()
        };

        // Get dept_roles
        let dept_roles = AuthRepo::get_user_dept_roles(self.pool.as_ref(), user.user_id).await?;

        // Preserve the current_department_id from old token, or resolve if missing
        let current_department_id = if old_claims.current_department_id.is_some() {
            old_claims.current_department_id
        } else {
            self.resolve_default_department(&dept_roles).await?
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
            system_role,
            dept_roles,
            current_department_id,
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

        let system_role = if user.is_super_admin {
            "super_admin".to_string()
        } else {
            "user".to_string()
        };

        let dept_roles = AuthRepo::get_user_dept_roles(self.pool.as_ref(), user.user_id).await?;
        let current_department_id = self.resolve_default_department(&dept_roles).await?;

        let display_name = user.display_name.clone().unwrap_or_default();
        Ok(Claims {
            sub: user.user_id,
            username: user.username,
            display_name,
            system_role,
            dept_roles,
            current_department_id,
            exp: 0,
            iat: 0,
        })
    }

    fn list_resources(&self) -> Vec<ResourceActionDef> {
        self.resource_actions.clone()
    }

    async fn switch_department(&self, user_id: i64, department_id: i64) -> Result<(String, i64, Claims)> {
        // 1. Verify user exists and is active
        let user = AuthRepo::find_user_by_id(self.pool.as_ref(), user_id)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;
        if !user.is_active {
            return Err(anyhow!("User account is disabled"));
        }

        // 2. Verify user belongs to this department
        let dept_roles = AuthRepo::get_user_dept_roles(self.pool.as_ref(), user_id).await?;
        if !dept_roles.contains_key(&department_id.to_string()) {
            return Err(anyhow!("User does not belong to department {}", department_id));
        }

        // 3. Build new claims with updated current_department_id
        let system_role = if user.is_super_admin { "super_admin" } else { "user" }.to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let display_name = user.display_name.clone().unwrap_or_default();
        let claims = Self::build_claims(
            user.user_id,
            user.username.clone(),
            display_name,
            system_role,
            dept_roles,
            Some(department_id),
            now,
            self.jwt_expiration_hours,
        );

        let expires_at = claims.exp as i64;
        let token = self.sign_jwt(&claims)?;
        Ok((token, expires_at, claims))
    }
}
