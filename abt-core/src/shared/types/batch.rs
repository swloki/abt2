use super::error::DomainError;

/// 批量操作模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchMode {
    /// 全建或全不建
    Atomic,
    /// 部分失败继续
    ContinueOnError,
}

/// 批量操作失败项
#[derive(Debug)]
pub struct BatchFailure {
    pub index: i32,
    pub error: DomainError,
}

/// 批量操作统一返回
#[derive(Debug)]
pub struct BatchResult {
    pub success_count: i32,
    pub failed_items: Vec<BatchFailure>,
    pub total: i32,
    pub mode: BatchMode,
}

impl BatchResult {
    pub fn atomic_ok(count: i32) -> Self {
        Self {
            success_count: count,
            failed_items: vec![],
            total: count,
            mode: BatchMode::Atomic,
        }
    }

    pub fn continue_on_error(
        success_count: i32,
        failed_items: Vec<BatchFailure>,
        total: i32,
    ) -> Self {
        Self {
            success_count,
            failed_items,
            total,
            mode: BatchMode::ContinueOnError,
        }
    }

    pub fn all_failed(total: i32, errors: Vec<DomainError>) -> Self {
        let failed_items: Vec<BatchFailure> = errors
            .into_iter()
            .enumerate()
            .map(|(i, e)| BatchFailure {
                index: i as i32,
                error: e,
            })
            .collect();
        Self {
            success_count: 0,
            total,
            failed_items,
            mode: BatchMode::ContinueOnError,
        }
    }
}
