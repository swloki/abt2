//! MES 生产制造执行模块
//!
//! 覆盖从生产计划到完工入库的完整生产管理流程。
//! 严格遵循 docs/uml-design/04-mes.html 中的 UML 设计。

pub mod enums;

pub mod production_plan;
pub mod work_order;
pub mod production_batch;
pub mod work_report;
pub mod production_inspection;
pub mod production_receipt;
pub mod dashboard;

pub use enums::*;
