use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BomCategory {
    pub bom_category_id: i64,
    pub bom_category_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBomCategoryRequest {
    pub bom_category_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBomCategoryRequest {
    pub bom_category_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomCategoryQuery {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl Default for BomCategoryQuery {
    fn default() -> Self {
        Self {
            keyword: None,
            page: Some(1),
            page_size: Some(20),
        }
    }
}
