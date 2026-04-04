use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Department {
    pub department_id: i64,
    pub department_name: String,
    pub department_code: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Department {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Department {
            department_id: row.try_get("department_id")?,
            department_name: row.try_get("department_name")?,
            department_code: row.try_get("department_code")?,
            description: row.try_get("description")?,
            is_active: row.try_get("is_active")?,
            is_default: row.try_get("is_default")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDepartmentRequest {
    pub department_name: String,
    pub department_code: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateDepartmentRequest {
    pub department_name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}
