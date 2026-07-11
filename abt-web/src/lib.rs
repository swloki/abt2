// axum handler / repo 函数天然参数较多（State + 多 extractor + Form/Query），非设计缺陷
#![allow(clippy::too_many_arguments)]
// 前端 handler 偶有复杂组合类型，拆 type alias 反而降低可读性
#![allow(clippy::type_complexity)]

pub mod auth;
pub mod components;
pub mod config;
pub mod errors;
pub mod layout;
pub mod middleware;
pub mod pages;
pub mod permissions;
pub mod routes;
pub mod state;
pub mod toast;
pub mod utils;
