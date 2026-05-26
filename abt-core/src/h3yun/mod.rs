//! H3Yun ERP 同步模块
//!
//! 单向同步 ABT 产品和库存数据到 H3Yun ERP。
//! 通过 EventHandler 监听领域事件自动触发同步。

pub mod client;
pub mod handlers;
pub mod inventory_sync;
pub mod models;
pub mod product_sync;
pub mod sync_state;

pub use client::H3YunClient;
pub use models::{EntityType, SyncError, SyncState};
pub use handlers::{ProductSyncHandler, ProductDeleteHandler, InventorySyncHandler};
