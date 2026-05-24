use async_trait::async_trait;

use super::model::{Claims, ResourceActionDef};
use crate::shared::types::error::DomainError;

#[async_trait]
pub trait AuthService: Send + Sync {
    /// Validate username/password, return JWT token and Claims
    async fn login(&self, username: &str, password: &str) -> Result<(String, Claims), DomainError>;

    /// Refresh a valid JWT token (re-emit with new expiry)
    async fn refresh_token(&self, token: &str) -> Result<String, DomainError>;

    /// Build Claims for a given user_id
    async fn get_user_claims(&self, user_id: i64) -> Result<Claims, DomainError>;

    /// List all defined resource/action permission entries
    fn list_resources(&self) -> Vec<ResourceActionDef>;
}
