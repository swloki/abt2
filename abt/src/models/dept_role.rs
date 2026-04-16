use serde::{Deserialize, Serialize};

/// 用户在某个部门的角色分配
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptRole {
    pub department_id: i64,
    pub role_id: i64,
}

/// 用户在某个部门的角色分配（含部门名称和角色名称，用于 API 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptRoleDetail {
    pub department_id: i64,
    pub department_name: String,
    pub role_id: i64,
    pub role_name: String,
}

/// 分配用户部门角色的请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignDeptRolesRequest {
    pub assignments: Vec<DeptRole>,
}
