//! abt-core — ABT 核心业务库（按业务域组织）
//!
//! 按模块化分层架构组织，业务模块单向依赖 shared 层。

// axum handler / repo 函数天然参数较多（State + 多 extractor + Form/Query），非设计缺陷
#![allow(clippy::too_many_arguments)]
// 前端 handler 偶有复杂组合类型，拆 type alias 反而降低可读性
#![allow(clippy::type_complexity)]

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
