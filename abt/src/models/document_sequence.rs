//! 单据序号数据模型
//!
//! 包含单据序号实体定义，用于生成格式化的单据编号。

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 单据序号实体
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentSequence {
    pub sequence_id: i64,
    pub doc_type: String,
    pub prefix: String,
    pub current_value: i32,
    pub reset_rule: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl<'r> FromRow<'r, PgRow> for DocumentSequence {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(DocumentSequence {
            sequence_id: row.try_get("sequence_id")?,
            doc_type: row.try_get("doc_type")?,
            prefix: row.try_get("prefix")?,
            current_value: row.try_get("current_value")?,
            reset_rule: row.try_get("reset_rule")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
