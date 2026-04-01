use serde::{Deserialize, Serialize};

/// JWT Claims 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 用户 ID
    pub sub: i64,
    /// 用户名
    pub username: String,
    /// 显示名
    pub display_name: String,
    /// 是否超级管理员
    pub is_super_admin: bool,
    /// 权限列表 ["product:read", "product:write", ...]
    pub permissions: Vec<String>,
    /// 过期时间 (UNIX timestamp)
    pub exp: u64,
    /// 签发时间 (UNIX timestamp)
    pub iat: u64,
}

/// 从 gRPC request extensions 中提取的认证上下文
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: i64,
    pub username: String,
    pub is_super_admin: bool,
    pub permissions: Vec<String>,
}

impl AuthContext {
    /// 检查权限。super_admin 自动通过。
    pub fn check_permission(&self, resource: &str, action: &str) -> Result<(), String> {
        if self.is_super_admin {
            return Ok(());
        }
        let required = format!("{}:{}", resource, action);
        if self.permissions.contains(&required) {
            Ok(())
        } else {
            Err(format!("No permission for {}:{}", resource, action))
        }
    }
}

/// 资源操作定义（代码注册，非数据库）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceActionDef {
    pub resource_code: &'static str,
    pub resource_name: &'static str,
    pub description: &'static str,
    pub action: &'static str,
    pub action_name: &'static str,
}
