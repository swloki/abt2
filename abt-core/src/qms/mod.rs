//! 质量管理 QMS 模块
//!
//! 三道质量关口（IQC/IPQC/FQC-OQC）+ MRB 不良评审 + RMA 客诉追溯
//! 设计文档: docs/uml-design/06-qms.html v2.3

pub mod enums;
pub mod inspection_specification;
pub mod inspection_result;
pub mod mrb;
pub mod quality_gate;
pub mod rma;

pub use quality_gate::QualityGateService;
