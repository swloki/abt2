//! 单据序号数据访问层
//!
//! 提供单据序号的数据库操作。

use anyhow::{Context, Result};
use chrono::Utc;

use crate::models::DocumentSequence;
use crate::repositories::Executor;

/// 单据序号数据仓库
pub struct DocumentSequenceRepo;

impl DocumentSequenceRepo {
    /// 获取下一个单据编号
    ///
    /// 使用 `SELECT ... FOR UPDATE` 锁定行，确保并发安全。
    /// 如果 `updated_at` 跨月则重置 `current_value` 为 0。
    /// 格式：`{prefix}{year}-{month}-{5位序号}`，例如 `PO-2026-05-00001`
    pub async fn next_number(executor: Executor<'_>, doc_type: &str) -> Result<String> {
        // 锁定行并读取当前值
        let row = sqlx::query_as::<_, DocumentSequence>(
            r#"
            SELECT sequence_id, doc_type, prefix, current_value, reset_rule, created_at, updated_at
            FROM document_sequences
            WHERE doc_type = $1
            FOR UPDATE
            "#,
        )
        .bind(doc_type)
        .fetch_one(&mut *executor)
        .await
        .with_context(|| format!("单据类型 '{}' 未找到", doc_type))?;

        let now = Utc::now();
        let mut current_value = row.current_value;

        // 检查是否需要按月重置
        if row.reset_rule == "monthly" {
            let updated_month = row.updated_at.format("%Y-%m").to_string();
            let current_month = now.format("%Y-%m").to_string();
            if updated_month != current_month {
                current_value = 0;
            }
        }

        current_value += 1;

        // 更新序号值和时间戳
        sqlx::query(
            r#"
            UPDATE document_sequences
            SET current_value = $1, updated_at = NOW()
            WHERE doc_type = $2
            "#,
        )
        .bind(current_value)
        .bind(doc_type)
        .execute(executor)
        .await?;

        // 格式化单据编号：{prefix}{year}-{month}-{5位序号}
        let formatted = format!(
            "{}{}-{}-{:05}",
            row.prefix,
            now.format("%Y"),
            now.format("%m"),
            current_value
        );

        Ok(formatted)
    }

    /// 确保单据序号记录存在（不存在则插入）
    pub async fn ensure_sequence(
        executor: Executor<'_>,
        doc_type: &str,
        prefix: &str,
        reset_rule: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
            VALUES ($1, $2, 0, $3)
            ON CONFLICT (doc_type) DO NOTHING
            "#,
        )
        .bind(doc_type)
        .bind(prefix)
        .bind(reset_rule)
        .execute(executor)
        .await?;

        Ok(())
    }
}
