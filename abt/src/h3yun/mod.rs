//! H3Yun ERP 同步模块
//!
//! 单向同步 ABT 产品和库存数据到 H3Yun ERP。

pub mod client;
pub mod inventory_sync;
pub mod models;
pub mod product_sync;
pub mod scheduled;
pub mod sync_state;
pub mod sync_worker;

use models::SyncEvent;
use std::sync::OnceLock;
use tokio::sync::mpsc::Sender;

/// 全局 SyncEvent channel sender
static SYNC_SENDER: OnceLock<Sender<SyncEvent>> = OnceLock::new();

/// 获取全局 SyncEvent sender，用于各触发源发送同步事件
pub fn get_sync_event_sender() -> &'static Sender<SyncEvent> {
    SYNC_SENDER
        .get()
        .expect("Sync event sender not initialized. Call start_sync_channel() first.")
}

/// 设置全局 sender（仅在 start_sync_channel 时调用）
#[allow(dead_code)]
pub(crate) fn set_sync_event_sender(sender: Sender<SyncEvent>) {
    SYNC_SENDER
        .set(sender)
        .expect("Sync event sender already initialized");
}

/// 检查 sender 是否已初始化（避免删除流程中未初始化时 panic）
pub fn has_sync_event_sender() -> bool {
    SYNC_SENDER.get().is_some()
}
