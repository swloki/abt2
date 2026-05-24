use chrono::{DateTime, NaiveDate, Utc};

use super::super::enums::SequenceStrategy;

/// 单据编号序列 — 按日期分段原子递增
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DocumentSequence {
    pub id: i64,
    pub prefix: String,
    pub current_value: i32,
    pub seq_date: NaiveDate,
    pub padding_len: i32,
    pub strategy: SequenceStrategy,
    pub created_at: DateTime<Utc>,
}
