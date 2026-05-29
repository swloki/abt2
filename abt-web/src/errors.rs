use abt_core::shared::types::DomainError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};

#[derive(Debug)]
pub struct WebError(DomainError);

pub type Result<T, E = WebError> = std::result::Result<T, E>;

impl From<DomainError> for WebError {
    fn from(e: DomainError) -> Self {
        Self(e)
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, title, message) = match &self.0 {
            DomainError::NotFound(msg) => (StatusCode::NOT_FOUND, "页面未找到", msg.clone()),
            DomainError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "未授权", msg.clone()),
            DomainError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, "权限不足", msg.clone()),
            DomainError::Duplicate(msg)
            | DomainError::Validation(msg)
            | DomainError::BusinessRule(msg) => (StatusCode::BAD_REQUEST, "请求错误", msg.clone()),
            DomainError::InvalidStateTransition { from, to } => {
                (StatusCode::BAD_REQUEST, "请求错误", format!("状态转换无效: {from} -> {to}"))
            }
            DomainError::ConcurrentConflict => {
                (StatusCode::CONFLICT, "并发冲突", "数据已被其他操作修改，请刷新后重试".into())
            }
            DomainError::Internal(e) => {
                tracing::error!("Internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "服务器错误", "服务器内部错误，请稍后重试".into())
            }
        };

        let html = error_page(title, &message);
        (status, axum::response::Html(html.into_string())).into_response()
    }
}

#[allow(dead_code)]
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
