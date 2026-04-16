use anyhow::Result;
use async_trait::async_trait;

use crate::models::{Claims, ResourceActionDef};

#[async_trait]
pub trait AuthService: Send + Sync {
    /// 用户名密码登录，返回 JWT token
    async fn login(&self, username: &str, password: &str) -> Result<(String, i64, Claims)>;

    /// 刷新 token
    async fn refresh_token(&self, token: &str) -> Result<(String, i64, Claims)>;

    /// 根据 user_id 获取 Claims（用于 GetCurrentUser）
    async fn get_user_claims(&self, user_id: i64) -> Result<Claims>;

    /// 获取所有资源定义
    fn list_resources(&self) -> Vec<ResourceActionDef>;

    /// Switch current department context, returns updated token
    async fn switch_department(&self, user_id: i64, department_id: i64) -> Result<(String, i64, Claims)>;
}
