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
