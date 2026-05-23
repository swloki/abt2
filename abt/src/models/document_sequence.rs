use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

#[derive(Debug, Clone)]
pub struct DocumentSequence {
    pub sequence_id: i64,
    pub doc_type: String,
    pub prefix: String,
    pub current_value: i32,
    pub reset_rule: String,
    pub last_reset_at: chrono::NaiveDate,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, PgRow> for DocumentSequence {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(DocumentSequence {
            sequence_id: row.try_get("sequence_id")?,
            doc_type: row.try_get("doc_type")?,
            prefix: row.try_get("prefix")?,
            current_value: row.try_get("current_value")?,
            reset_rule: row.try_get("reset_rule")?,
            last_reset_at: row.try_get("last_reset_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
