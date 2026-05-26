use std::backtrace::Backtrace;
use tonic::Code;
use tonic_types::{ErrorDetails, StatusExt as _};
use abt_core::shared::types::DomainError;

/// Convert DomainError to tonic::Status with structured error details.
pub fn domain_err(e: DomainError) -> tonic::Status {
    match e {
        DomainError::NotFound(entity) => not_found(&entity, ""),
        DomainError::Duplicate(entity) => {
            let mut details = ErrorDetails::new();
            details.set_error_info("ALREADY_EXISTS", "abt.api", std::collections::HashMap::new());
            tonic::Status::with_error_details(Code::AlreadyExists, entity, details)
        }
        DomainError::PermissionDenied(msg) => forbidden("", &msg),
        DomainError::Validation(msg) => validation("", &msg),
        DomainError::BusinessRule(msg) => business_error("_business", &msg),
        DomainError::ConcurrentConflict => tonic::Status::aborted("Concurrent conflict"),
        DomainError::InvalidStateTransition { from, to } => {
            validation("state", &format!("Cannot transition from {from} to {to}"))
        }
        DomainError::Internal(inner) => err_to_status(inner),
    }
}

/// Convert anyhow::Error to tonic::Status while logging the full error with backtrace.
pub fn err_to_status(e: anyhow::Error) -> tonic::Status {
    let mut msg = e.to_string();
    let mut source = e.source();
    while let Some(cause) = source {
        msg.push_str(&format!("\n  Caused by: {}", cause));
        source = cause.source();
    }
    let bt = Backtrace::capture();
    tracing::error!("{}\n\n  Debug: {:?}\n\n{}", msg, e, clean_backtrace(&bt));
    tonic::Status::internal(msg)
}

/// Convert sqlx::Error to tonic::Status.
pub fn sqlx_err_to_status(e: sqlx::Error) -> tonic::Status {
    err_to_status(anyhow::Error::from(e))
}

/// Create a validation error for a single field.
pub fn validation(field: &str, message: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.add_bad_request_violation(field, message);
    tracing::warn!("Validation error: {} - {}", field, message);
    tonic::Status::with_error_details(Code::InvalidArgument, "Validation failed", details)
}

/// Create a validation error for multiple fields.
pub fn validation_errors(errors: Vec<(&str, &str)>) -> tonic::Status {
    let mut details = ErrorDetails::new();
    for e in &errors {
        details.add_bad_request_violation(e.0, e.1);
    }
    tracing::warn!("Validation errors: {:?}", errors);
    tonic::Status::with_error_details(Code::InvalidArgument, "Validation failed", details)
}

/// Create a resource not found error.
pub fn not_found(resource_type: &str, resource_name: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.set_resource_info(resource_type, "", resource_name, "");
    tracing::warn!("Not found: {} {}", resource_type, resource_name);
    tonic::Status::with_error_details(Code::NotFound, format!("{} not found", resource_type), details)
}

/// Create a resource conflict error.
pub fn conflict(resource: &str, field: &str, value: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.set_error_info(
        "ALREADY_EXISTS",
        "abt.api",
        std::collections::HashMap::from([
            ("resource".to_string(), resource.to_string()),
            ("field".to_string(), field.to_string()),
            ("value".to_string(), value.to_string()),
        ]),
    );
    tracing::warn!("Conflict: {} {}='{}'", resource, field, value);
    tonic::Status::with_error_details(Code::AlreadyExists, format!("{} already exists", resource), details)
}

/// Create a permission denied error.
pub fn forbidden(resource: &str, action: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.set_error_info(
        "PERMISSION_DENIED",
        "abt.api",
        std::collections::HashMap::from([
            ("resource".to_string(), resource.to_string()),
            ("action".to_string(), action.to_string()),
        ]),
    );
    tracing::warn!("Forbidden: {} {}", resource, action);
    tonic::Status::with_error_details(Code::PermissionDenied, format!("No permission to {} {}", action, resource), details)
}

/// Create an unauthenticated error.
pub fn unauthorized(message: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.set_error_info("UNAUTHENTICATED", "abt.api", std::collections::HashMap::new());
    tonic::Status::with_error_details(Code::Unauthenticated, message, details)
}

/// Business validation error.
pub fn business_error(field: &str, message: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.add_bad_request_violation(field, message);
    tonic::Status::with_error_details(Code::FailedPrecondition, message, details)
}

fn clean_backtrace(bt: &Backtrace) -> String {
    let s = bt.to_string();
    let lines: Vec<&str> = s.lines().skip(5).collect();
    lines.join("\n")
}
