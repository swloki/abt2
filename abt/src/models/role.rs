use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Role {
    pub role_id: i64,
    pub role_name: String,
    pub role_code: String,
    pub is_system_role: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Role {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Role {
            role_id: row.try_get("role_id")?,
            role_name: row.try_get("role_name")?,
            role_code: row.try_get("role_code")?,
            is_system_role: row.try_get("is_system_role")?,
            description: row.try_get("description")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleWithPermissions {
    pub role: Role,
    pub permissions: Vec<PermissionInfo>,
}

use super::permission::PermissionInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub role_name: String,
    pub role_code: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRoleRequest {
    pub role_name: Option<String>,
    pub description: Option<String>,
}
