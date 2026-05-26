use sqlx::query_as;
use crate::shared::types::RepoResult;

use super::model::DocumentSequence;

pub struct DocumentSequenceRepo;

impl DocumentSequenceRepo {
    /// 原子 upsert：首次插入 current_value=1，后续每次 +1。
    /// UNIQUE 约束在 (prefix, seq_date)，天然按月分段。
    pub async fn next_sequential(
        executor: &mut sqlx::postgres::PgConnection,
        prefix: &str,
        padding_len: i32,
    ) -> RepoResult<DocumentSequence> {
        query_as::<_, DocumentSequence>(
            r#"
            INSERT INTO document_sequences (prefix, seq_date, current_value, padding_len, strategy)
            VALUES ($1, CURRENT_DATE, 1, $2, 1)
            ON CONFLICT (prefix, seq_date) DO UPDATE
            SET current_value = document_sequences.current_value + 1,
                updated_at = NOW()
            RETURNING id, prefix, current_value, seq_date, padding_len, strategy, created_at
            "#,
        )
        .bind(prefix)
        .bind(padding_len)
        .fetch_one(executor)
        .await
        .map_err(Into::into)
    }
}
