//! Excel 进度追踪器
//!
//! 独立于业务逻辑的进度追踪基础设施。
//! 导入器持有 `Arc<ProgressTracker>` 更新进度，handler 持有同一个 Arc 用于 `GetProgress` RPC。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::service::ExcelProgress;

/// 进度追踪器
///
/// 使用 `Arc<ProgressTracker>` 在导入器和 handler 之间共享进度状态。
pub struct ProgressTracker {
    current: AtomicUsize,
    total: AtomicUsize,
}

impl ProgressTracker {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            current: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
        })
    }

    pub fn set_total(&self, n: usize) {
        self.total.store(n, Ordering::Relaxed);
    }

    pub fn tick(&self) {
        self.current.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> ExcelProgress {
        ExcelProgress {
            current: self.current.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
        }
    }
}
