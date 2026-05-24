//! abt-core — ABT 核心业务库（按业务域组织）
//!
//! 按模块化分层架构组织，业务模块单向依赖 shared 层。

pub mod shared;
pub mod sales;
pub mod master_data;

// 未来模块占位
pub mod fms;
pub mod mes;
pub mod om;
pub mod purchase;
pub mod qms;
pub mod wms;
pub mod workflow;
