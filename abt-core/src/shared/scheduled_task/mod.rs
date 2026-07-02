//! 定时任务框架（预留未接入：trait/repo/model 已定义，暂无任务实现与调度器接入）。
#![allow(dead_code)]

pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::{ScheduledTask, TaskSchedulerService};
