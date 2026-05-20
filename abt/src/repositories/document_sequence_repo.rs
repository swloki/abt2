//! 单据序号数据访问层
//!
//! 提供单据序号的数据库操作，包括生成下一个编号和确保序号记录存在。

use anyhow::Result;

use crate::models::DocumentSequence;
use crate::repositories::Executor;

/// 单据序号数据仓库
pub struct DocumentSequenceRepo;

impl DocumentSequenceRepo {
    /// 生成下一个单据编号。使用 SELECT FOR UPDATE 保证并发安全。
    ///
    /// 格式:
    /// - "monthly": `{prefix}{YYYY-MM}-{NNNNN}` 例如 `"QT2026-05-00001"`
    /// - "yearly": `{prefix}{YYYY}-{NNNNN}` 例如 `"QT2026-00001"`
    /// - 其他: `{prefix}-{NNNNN}` 例如 `"QT-00001"`
    pub async fn next_number(executor: Executor<'_>, doc_type: &str) -> Result<String> {
        let seq: DocumentSequence = sqlx::query_as(
            "SELECT sequence_id, doc_type, prefix, current_value, reset_rule, created_at, updated_at \
             FROM document_sequences WHERE doc_type = $1 FOR UPDATE",
        )
        .bind(doc_type)
        .fetch_one(&mut *executor)
        .await?;

        let new_value = seq.current_value + 1;
        let now = chrono::Utc::now();

        sqlx::query(
            "UPDATE document_sequences SET current_value = $1, updated_at = NOW() WHERE doc_type = $2",
        )
        .bind(new_value)
        .bind(doc_type)
        .execute(&mut *executor)
        .await?;

        let number = match seq.reset_rule.as_str() {
            "monthly" => format!("{}{}-{:05}", seq.prefix, now.format("%Y-%m"), new_value),
            "yearly" => format!("{}{}-{:05}", seq.prefix, now.format("%Y"), new_value),
            _ => format!("{}-{:05}", seq.prefix, new_value),
        };

        Ok(number)
    }

    /// 确保指定单据类型的序号记录存在。INSERT ON CONFLICT DO NOTHING。
    pub async fn ensure_sequence(
        executor: Executor<'_>,
        doc_type: &str,
        prefix: &str,
        reset_rule: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule) \
             VALUES ($1, $2, 0, $3) \
             ON CONFLICT (doc_type) DO NOTHING",
        )
        .bind(doc_type)
        .bind(prefix)
        .bind(reset_rule)
        .execute(&mut *executor)
        .await?;

        Ok(())
    }
}
