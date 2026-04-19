//! gRPC Rich Error helpers
//!
//! Provides structured error responses following Google AIP-193 standard.
//! Frontend can extract structured details via ConnectRPC's `findDetails()`.

use std::backtrace::Backtrace;
use tonic::Code;
use tonic_types::{ErrorDetails, StatusExt};

/// Convert anyhow::Error to tonic::Status while logging the full error with backtrace.
///
/// Used for runtime errors (database, IO, etc.) without structured details.
/// Frontend receives a generic message and logs to monitoring.
pub fn err_to_status(e: anyhow::Error) -> tonic::Status {
    if let Some(se) = e.downcast_ref::<ServiceError>() {
        return se.to_status();
    }
    let mut msg = e.to_string();
    let mut source = e.source();
    while let Some(cause) = source {
        msg.push_str(&format!("\n  Caused by: {}", cause));
        source = cause.source();
    }
    let bt = Backtrace::capture();
    tracing::error!(
        "{}\n\n  Debug: {:?}\n\n{}",
        msg,
        e,
        clean_backtrace(&bt)
    );
    tonic::Status::internal(msg)
}

/// Convert sqlx::Error to tonic::Status while logging the full error with backtrace.
pub fn sqlx_err_to_status(e: sqlx::Error) -> tonic::Status {
    err_to_status(anyhow::Error::from(e))
}

// ─── Business error helper functions ─────────────────────────────────────
// Each function returns a tonic::Status with rich error details.
// Frontend extracts structured data via ConnectRPC's findDetails().

/// Create a validation error for a single field.
///
/// ```rust
/// return Err(common::error::validation("name", "Name is required"));
/// ```
pub fn validation(field: &str, message: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.add_bad_request_violation(field, message);
    tracing::warn!("Validation error: {} - {}", field, message);
    tonic::Status::with_error_details(
        Code::InvalidArgument,
        "Validation failed",
        details,
    )
}

/// Create a validation error for multiple fields.
///
/// ```rust
/// return Err(common::error::validation_errors(vec![
///     ("code", "Code must be at least 2 characters"),
///     ("name", "Name is required"),
/// ]));
/// ```
pub fn validation_errors(errors: Vec<(&str, &str)>) -> tonic::Status {
    let mut details = ErrorDetails::new();
    for e in &errors {
        details.add_bad_request_violation(e.0, e.1);
    }
    tracing::warn!("Validation errors: {:?}", errors);
    tonic::Status::with_error_details(
        Code::InvalidArgument,
        "Validation failed",
        details,
    )
}

/// Create a resource not found error.
///
/// ```rust
/// return Err(common::error::not_found("Menu", &id.to_string()));
/// ```
pub fn not_found(resource_type: &str, resource_name: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.set_resource_info(resource_type, "", resource_name, "");
    tracing::warn!("Not found: {} {}", resource_type, resource_name);
    tonic::Status::with_error_details(
        Code::NotFound,
        format!("{} not found", resource_type),
        details,
    )
}

/// Create a resource conflict error (unique constraint violation).
///
/// ```rust
/// return Err(common::error::conflict("Language", "code", "fr"));
/// ```
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
    tonic::Status::with_error_details(
        Code::AlreadyExists,
        format!("{} already exists", resource),
        details,
    )
}

/// Create a permission denied error.
///
/// ```rust
/// return Err(common::error::forbidden("Menu", "delete"));
/// ```
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
    tonic::Status::with_error_details(
        Code::PermissionDenied,
        format!("No permission to {} {}", action, resource),
        details,
    )
}

/// Create an unauthenticated error.
///
/// ```rust
/// return Err(common::error::unauthorized("Login expired"));
/// ```
pub fn unauthorized(message: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.set_error_info(
        "UNAUTHENTICATED",
        "abt.api",
        std::collections::HashMap::new(),
    );
    tonic::Status::with_error_details(Code::Unauthenticated, message, details)
}

/// Business validation error - returned to frontend without console logging.
///
/// Use for expected validation failures (not bugs or infrastructure errors).
/// Frontend receives a structured error via ConnectRPC's findDetails().
pub fn business_error(field: &str, message: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.add_bad_request_violation(field, message);
    tonic::Status::with_error_details(
        Code::InvalidArgument,
        message,
        details,
    )
}

// ─── Utility ──────────────────────────────────────────────────────────────

/// Clean up backtrace for logging (removes noisy frames).
fn clean_backtrace(bt: &Backtrace) -> String {
    let s = bt.to_string();
    // Remove the first few frames which are always noise from this module
    let lines: Vec<&str> = s.lines().skip(5).collect();
    lines.join("\n")
}

// ─── Service-layer error types ────────────────────────────────────────────

/// Structured errors from the service layer that map to specific HTTP status codes.
///
/// Service implementations return these via `Err(anyhow::Error::from(ServiceError::...))`.
/// The `err_to_status` function downcasts and converts to the appropriate `tonic::Status`.
pub enum ServiceError {
    NotFound { resource: String, id: String },
    Conflict { resource: String, message: String },
    BusinessValidation { message: String },
}

impl ServiceError {
    fn to_status(&self) -> tonic::Status {
        match self {
            Self::NotFound { resource, id } => not_found(resource, id),
            Self::Conflict { resource, message } => {
                let mut details = ErrorDetails::new();
                details.set_error_info(
                    "ALREADY_EXISTS",
                    "abt.api",
                    std::collections::HashMap::from([
                        ("resource".to_string(), resource.clone()),
                    ]),
                );
                tracing::warn!("Conflict: {} - {}", resource, message);
                tonic::Status::with_error_details(
                    Code::AlreadyExists,
                    message.clone(),
                    details,
                )
            }
            Self::BusinessValidation { message } => {
                let mut details = ErrorDetails::new();
                details.add_bad_request_violation("_business", message);
                tonic::Status::with_error_details(
                    Code::FailedPrecondition,
                    message.clone(),
                    details,
                )
            }
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { resource, id } => write!(f, "{} {} not found", resource, id),
            Self::Conflict { resource, message } => write!(f, "{}: {}", resource, message),
            Self::BusinessValidation { message } => write!(f, "{}", message),
        }
    }
}

impl std::fmt::Debug for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { resource, id } => f.debug_struct("NotFound").field("resource", resource).field("id", id).finish(),
            Self::Conflict { resource, message } => f.debug_struct("Conflict").field("resource", resource).field("message", message).finish(),
            Self::BusinessValidation { message } => f.debug_struct("BusinessValidation").field("message", message).finish(),
        }
    }
}

impl std::error::Error for ServiceError {}
