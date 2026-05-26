use std::sync::Arc;

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use async_trait::async_trait;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sqlx::postgres::PgPool;

use super::super::auth_service::AuthService;
use super::super::model::{Claims, ResourceActionDef};
use super::super::repo::IdentityRepo;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

const JWT_ISSUER: &str = "abt-core";

pub struct AuthServiceImpl {
    pool: Arc<PgPool>,
    jwt_secret: String,
}

impl AuthServiceImpl {
    pub fn new(pool: Arc<PgPool>, jwt_secret: String) -> Self {
        Self { pool, jwt_secret }
    }
}

#[async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(&self, username: &str, password: &str) -> Result<(String, Claims)> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let user = IdentityRepo::get_user_by_username(&mut conn, username)
            .await
            .map_err(|e| match &e {
                DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"),
                _ => e,
            })?;

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|e| DomainError::Internal(anyhow::anyhow!("argon2 parse error: {e}")))?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| DomainError::permission_denied("Invalid credentials"))?;

        let claims = self.build_claims(&mut conn, &user).await?;

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok((token, claims))
    }

    async fn refresh_token(&self, token: &str) -> Result<String> {
        let mut validation = Validation::default();
        validation.validate_exp = false;
        validation.set_issuer(&[JWT_ISSUER]);

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|e| DomainError::Internal(e.into()))?;

        let claims = self
            .get_user_claims(token_data.claims.sub)
            .await?;

        let new_token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(new_token)
    }

    async fn get_user_claims(&self, user_id: i64) -> Result<Claims> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let user = IdentityRepo::get_user(&mut conn, user_id)
            .await
            .map_err(|e| match &e {
                DomainError::Internal(inner) if is_no_row(inner) => DomainError::not_found("User"),
                _ => e,
            })?;

        self.build_claims(&mut conn, &user).await
    }

    fn list_resources(&self) -> Vec<ResourceActionDef> {
        super::super::model::RESOURCE_ACTION_DEFS.to_vec()
    }
}

impl AuthServiceImpl {
    async fn build_claims(
        &self,
        conn: &mut sqlx::postgres::PgConnection,
        user: &super::super::model::User,
    ) -> Result<Claims> {
        let role_ids = IdentityRepo::get_user_role_ids(conn, user.user_id).await?;
        let role_codes = IdentityRepo::get_user_role_codes(conn, user.user_id).await?;
        let department_ids = IdentityRepo::get_user_department_ids(conn, user.user_id).await?;

        let now = Utc::now();
        let exp = match now.checked_add_signed(chrono::Duration::hours(24)) {
            Some(t) => t.timestamp() as u64,
            None => u64::MAX,
        };

        let system_role = if user.is_super_admin {
            "super_admin".to_string()
        } else {
            "user".to_string()
        };

        Ok(Claims {
            sub: user.user_id,
            username: user.username.clone(),
            display_name: user.display_name.clone().unwrap_or_default(),
            system_role,
            role_ids,
            role_codes,
            department_ids,
            iss: JWT_ISSUER.to_string(),
            exp,
            iat: now.timestamp() as u64,
        })
    }
}

fn is_no_row(err: &anyhow::Error) -> bool {
    err.downcast_ref::<sqlx::Error>()
        .map(|e| matches!(e, sqlx::Error::RowNotFound))
        .unwrap_or(false)
}
