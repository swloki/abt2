use async_trait::async_trait;

use super::super::types::error::DomainError;
use super::super::types::Result;
use super::super::types::context::ServiceContext;
use super::super::enums::document_type::DocumentType;

#[async_trait]
pub trait DocumentSequenceService: Send + Sync {
    /// 根据单据类型生成下一个编号。
    /// - Sequential 策略（默认）：PREFIX-YYYY-MM-SEQ（如 SO-2026-05-00142）
    /// - Timestamp 策略（Product）：x+Unix 秒级时间戳（如 x1747891200）
    async fn next_number(
        &self,
        ctx: ServiceContext<'_>,
        doc_type: DocumentType,
    ) -> Result<String>;
}
