use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Resource {
    pub resource_id: i64,
    pub resource_name: String,
    pub resource_code: String,
    pub group_name: Option<String>,
    pub sort_order: Option<i32>,
    pub description: Option<String>,
    pub department_id: Option<i64>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Resource {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Resource {
            resource_id: row.try_get("resource_id")?,
            resource_name: row.try_get("resource_name")?,
            resource_code: row.try_get("resource_code")?,
            group_name: row.try_get("group_name")?,
            sort_order: row.try_get("sort_order")?,
            description: row.try_get("description")?,
            department_id: row.try_get("department_id")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_code: String,
    pub action_name: String,
    pub sort_order: Option<i32>,
    pub description: Option<String>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Action {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Action {
            action_code: row.try_get("action_code")?,
            action_name: row.try_get("action_name")?,
            sort_order: row.try_get("sort_order")?,
            description: row.try_get("description")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Permission {
    pub permission_id: i64,
    pub permission_name: String,
    pub resource_id: i64,
    pub action_code: String,
    pub sort_order: Option<i32>,
    pub description: Option<String>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Permission {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Permission {
            permission_id: row.try_get("permission_id")?,
            permission_name: row.try_get("permission_name")?,
            resource_id: row.try_get("resource_id")?,
            action_code: row.try_get("action_code")?,
            sort_order: row.try_get("sort_order")?,
            description: row.try_get("description")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionInfo {
    pub permission_id: i64,
    pub permission_name: String,
    pub resource_id: i64,
    pub resource_name: String,
    pub resource_code: String,
    pub group_name: Option<String>,
    pub resource_sort_order: Option<i32>,
    pub resource_description: Option<String>,
    pub action_code: String,
    pub action_name: String,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for PermissionInfo {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(PermissionInfo {
            permission_id: row.try_get("permission_id")?,
            permission_name: row.try_get("permission_name")?,
            resource_id: row.try_get("resource_id")?,
            resource_name: row.try_get("resource_name")?,
            resource_code: row.try_get("resource_code")?,
            group_name: row.try_get("group_name")?,
            resource_sort_order: row.try_get("resource_sort_order")?,
            resource_description: row.try_get("resource_description")?,
            action_code: row.try_get("action_code")?,
            action_name: row.try_get("action_name")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub log_id: i64,
    pub operator_id: Option<i64>,
    pub operator_name: Option<String>,
    pub target_type: String,
    pub target_id: i64,
    pub action: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for AuditLog {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(AuditLog {
            log_id: row.try_get("log_id")?,
            operator_id: row.try_get("operator_id")?,
            operator_name: row.try_get("operator_name")?,
            target_type: row.try_get("target_type")?,
            target_id: row.try_get("target_id")?,
            action: row.try_get("action")?,
            old_value: row.try_get("old_value")?,
            new_value: row.try_get("new_value")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceGroup {
    pub group_name: String,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionGroup {
    pub group_name: String,
    pub permissions: Vec<PermissionInfo>,
}
