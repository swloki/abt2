use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};

pub enum AppError {
    NotFound(String),
    Unauthorized,
    Forbidden(String),
    BadRequest(String),
    Internal(String),
}

impl std::fmt::Debug for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "NotFound: {msg}"),
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::Forbidden(msg) => write!(f, "Forbidden: {msg}"),
            Self::BadRequest(msg) => write!(f, "BadRequest: {msg}"),
            Self::Internal(msg) => write!(f, "Internal: {msg}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "未登录".into()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {msg}");
                (StatusCode::INTERNAL_SERVER_ERROR, format!("服务器错误: {msg}"))
            }
        };

        (status, message).into_response()
    }
}

impl From<abt_core::shared::types::DomainError> for AppError {
    fn from(e: abt_core::shared::types::DomainError) -> Self {
        use abt_core::shared::types::DomainError::*;
        match e {
            NotFound(msg) => AppError::NotFound(msg),
            Duplicate(msg) => AppError::BadRequest(msg),
            PermissionDenied(msg) => AppError::Forbidden(msg),
            Validation(msg) => AppError::BadRequest(msg),
            BusinessRule(msg) => AppError::BadRequest(msg),
            _ => AppError::Internal(e.to_string()),
        }
    }
}

pub fn error_page(title: &str, message: &str) -> Markup {
    html! {
        div class="flex items-center justify-center min-h-[60vh]" {
            div class="text-center" {
                h1 class="text-2xl font-bold text-slate-700" { (title) }
                p class="mt-2 text-slate-500" { (message) }
                a href="/admin" class="mt-4 inline-block text-primary-500 hover:text-primary-600" { "返回首页" }
            }
        }
    }
}
