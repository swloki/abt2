use serde::{Deserialize, Serialize};

/// JWT Claims 结构 (Global Roles)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 用户 ID
    pub sub: i64,
    /// 用户名
    pub username: String,
    /// 显示名
    pub display_name: String,
    /// 系统角色: "super_admin" | "user"
    pub system_role: String,
    /// 全局角色 ID 列表
    pub role_ids: Vec<i64>,
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
    pub system_role: String,
    /// 全局角色 ID 列表
    pub role_ids: Vec<i64>,
}

impl AuthContext {
    /// 是否超级管理员
    pub fn is_super_admin(&self) -> bool {
        self.system_role == "super_admin"
    }

    /// 检查用户是否拥有指定角色
    pub fn has_role(&self, role_id: i64) -> bool {
        self.role_ids.contains(&role_id)
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
