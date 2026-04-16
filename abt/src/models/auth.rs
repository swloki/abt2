use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// JWT Claims 结构 (Scoped Roles)
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
    /// 部门-角色映射: department_id (as string key) -> list of role_ids
    pub dept_roles: HashMap<String, Vec<i64>>,
    /// 当前部门上下文 ID
    pub current_department_id: Option<i64>,
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
    pub dept_roles: HashMap<String, Vec<i64>>,
    pub current_department_id: Option<i64>,
}

impl AuthContext {
    /// 是否超级管理员
    pub fn is_super_admin(&self) -> bool {
        self.system_role == "super_admin"
    }

    /// 检查用户是否属于指定部门
    pub fn belongs_to_department(&self, department_id: i64) -> bool {
        self.is_super_admin()
            || self.dept_roles.contains_key(&department_id.to_string())
    }

    /// 获取用户在指定部门的角色 ID 列表
    pub fn get_dept_role_ids(&self, department_id: i64) -> Vec<i64> {
        self.dept_roles
            .get(&department_id.to_string())
            .cloned()
            .unwrap_or_default()
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
