use anyhow::Result;
use chrono::{Datelike, Utc};
use sqlx::query_as;

use crate::models::DocumentSequence;
use crate::repositories::Executor;

pub struct DocumentSequenceRepo;

impl DocumentSequenceRepo {
    pub async fn ensure_sequence(
        executor: Executor<'_>,
        doc_type: &str,
        prefix: &str,
        reset_rule: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO document_sequences (doc_type, prefix, reset_rule)
            VALUES ($1, $2, $3)
            ON CONFLICT (doc_type) DO NOTHING
            "#,
            doc_type,
            prefix,
            reset_rule,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn next_number(executor: Executor<'_>, doc_type: &str) -> Result<String> {
        let seq: DocumentSequence = query_as(
            "SELECT * FROM document_sequences WHERE doc_type = $1 FOR UPDATE",
        )
        .bind(doc_type)
        .fetch_one(&mut *executor)
        .await?;

        let now = Utc::now();
        let needs_reset = match seq.reset_rule.as_str() {
            "monthly" => {
                seq.last_reset_at.year() != now.year() || seq.last_reset_at.month() != now.month()
            }
            "yearly" => seq.last_reset_at.year() != now.year(),
            _ => false,
        };

        let (new_value, new_last_reset) = if needs_reset {
            (1, now.date_naive())
        } else {
            (seq.current_value + 1, seq.last_reset_at)
        };

        sqlx::query!(
            r#"
            UPDATE document_sequences
            SET current_value = $1, last_reset_at = $2, updated_at = NOW()
            WHERE doc_type = $3
            "#,
            new_value,
            new_last_reset,
            doc_type,
        )
        .execute(executor)
        .await?;

        let formatted = match seq.reset_rule.as_str() {
            "monthly" => format!(
                "{}{:04}-{:02}-{:05}",
                seq.prefix,
                now.year(),
                now.month(),
                new_value,
            ),
            "yearly" => format!("{}{:04}-{:05}", seq.prefix, now.year(), new_value),
            _ => format!("{}{:05}", seq.prefix, new_value),
        };

        Ok(formatted)
    }
}
