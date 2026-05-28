use crate::shared::types::Result;

use super::model::DocumentSequence;
use super::super::enums::SequenceStrategy;

pub struct DocumentSequenceRepo;

impl DocumentSequenceRepo {
    /// 原子 upsert：首次插入 current_value=1，后续每次 +1。
    /// UNIQUE 约束在 (prefix, seq_date)，按月分段（seq_date 取当月 1 号）。
    pub async fn next_sequential(
        executor: &mut sqlx::postgres::PgConnection,
        prefix: &str,
        padding_len: i32,
    ) -> Result<DocumentSequence> {
        let row = sqlx::query!(
            r#"
            INSERT INTO document_sequences (prefix, seq_date, current_value, padding_len, strategy)
            VALUES ($1, DATE_TRUNC('month', CURRENT_DATE)::date, 1, $2, 1)
            ON CONFLICT (prefix, seq_date) DO UPDATE
            SET current_value = document_sequences.current_value + 1
            RETURNING id, prefix, current_value, seq_date, padding_len,
                      strategy as "strategy: i16", created_at
            "#,
            prefix,
            padding_len,
        )
        .fetch_one(executor)
        .await?;

        Ok(DocumentSequence {
            id: row.id,
            prefix: row.prefix,
            current_value: row.current_value,
            seq_date: row.seq_date,
            padding_len: row.padding_len,
            strategy: SequenceStrategy::from_i16(row.strategy)
                .ok_or_else(|| crate::shared::types::error::DomainError::Internal(
                    anyhow::anyhow!("unknown SequenceStrategy: {}", row.strategy)
                ))?,
            created_at: row.created_at,
        })
    }
}
