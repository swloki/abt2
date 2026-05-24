use std::fmt;

/// 统一错误模型 — 所有 abt-core 服务使用此类型
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("{0} not found")]
    NotFound(String),

    #[error("{0} already exists")]
    Duplicate(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Business rule: {0}")]
    BusinessRule(String),

    #[error("Validation: {0}")]
    Validation(String),

    #[error("Concurrent conflict")]
    ConcurrentConflict,

    #[error("Invalid state transition: {from} -> {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl DomainError {
    pub fn not_found(entity: impl fmt::Display) -> Self {
        Self::NotFound(entity.to_string())
    }

    pub fn duplicate(entity: impl fmt::Display) -> Self {
        Self::Duplicate(entity.to_string())
    }

    pub fn permission_denied(msg: impl fmt::Display) -> Self {
        Self::PermissionDenied(msg.to_string())
    }

    pub fn business_rule(msg: impl fmt::Display) -> Self {
        Self::BusinessRule(msg.to_string())
    }

    pub fn validation(msg: impl fmt::Display) -> Self {
        Self::Validation(msg.to_string())
    }
}
