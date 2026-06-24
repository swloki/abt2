use abt_core::shared::types::DomainError;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use maud::{html, Markup};

/// Inline error card for rendering inside admin_page layout (e.g., "储位未找到").
#[allow(dead_code)]
pub fn error_page(title: &str, message: &str) -> Markup {
    html! {
        div class="flex items-center justify-center min-h-[60vh]" {
            div class="text-center" {
                h1 class="text-2xl font-bold text-fg" { (title) }
                p class="mt-2 text-muted" { (message) }
                a href="/admin" class="mt-4 inline-block text-accent" { "返回首页" }
            }
        }
    }
}

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
        let (status, message) = match &self.0 {
            DomainError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            DomainError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            DomainError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            DomainError::Duplicate(msg)
            | DomainError::Validation(msg)
            | DomainError::BusinessRule(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            DomainError::InsufficientStock {
                product_id,
                warehouse_id,
                available,
                required,
            } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "库存不足：产品#{product_id} 在仓库#{warehouse_id} 可用量 {available}，本次需 {required}"
                ),
            ),
            DomainError::InvalidStateTransition { from, to } => {
                (StatusCode::BAD_REQUEST, format!("状态转换无效: {from} -> {to}"))
            }
            DomainError::ConcurrentConflict => {
                (StatusCode::CONFLICT, "数据已被其他操作修改，请刷新后重试".into())
            }
            DomainError::Internal(e) => {
                tracing::error!("Internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误，请稍后重试".into())
            }
        };

        // For 403 errors, return a styled HTML error page so the user sees
        // a proper page with navigation instead of raw text.
        // NOTE: NotFound (404) is excluded — for HTMX form submissions, the
        // global error handler reads responseText as a toast message. Returning
        // a full HTML page breaks the toast. Plain text is consumed correctly.
        if status == StatusCode::FORBIDDEN {
            let title = "无权访问";
            let icon_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"#;
            let html_page = format!(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: var(--bg, #f8f9fb); color: var(--fg, #1e293b); display: flex; align-items: center; justify-content: center; min-height: 100vh; }}
        :root {{ --bg: #f8f9fb; --fg: #1e293b; --muted: #64748b; --accent: #4f46e5; --border: #e2e8f0; }}
        .error-card {{ text-align: center; padding: 3rem 2rem; max-width: 420px; }}
        .error-icon {{ color: var(--muted); margin-bottom: 1.5rem; }}
        .error-code {{ font-size: 4rem; font-weight: 800; color: var(--border); line-height: 1; margin-bottom: 0.75rem; }}
        .error-title {{ font-size: 1.25rem; font-weight: 600; margin-bottom: 0.5rem; }}
        .error-msg {{ font-size: 0.875rem; color: var(--muted); margin-bottom: 2rem; word-break: break-all; }}
        .error-actions {{ display: flex; gap: 0.75rem; justify-content: center; }}
        .btn {{ display: inline-flex; align-items: center; gap: 0.5rem; padding: 0.5rem 1.25rem; border-radius: 0.5rem; font-size: 0.875rem; font-weight: 500; text-decoration: none; cursor: pointer; border: none; }}
        .btn-primary {{ background: var(--accent); color: #fff; }}
        .btn-primary:hover {{ opacity: 0.9; }}
        .btn-default {{ background: transparent; color: var(--fg); border: 1px solid var(--border); }}
        .btn-default:hover {{ background: var(--border); }}
    </style>
</head>
<body>
    <div class="error-card">
        <div class="error-icon">{icon_svg}</div>
        <div class="error-code">{status_code}</div>
        <div class="error-title">{title}</div>
        <div class="error-msg">{message}</div>
        <div class="error-actions">
            <a href="/admin" class="btn btn-primary">返回首页</a>
            <button onclick="history.back()" class="btn btn-default">返回上页</button>
        </div>
    </div>
</body>
</html>"#, status_code = status.as_u16(), title = title, message = message, icon_svg = icon_svg);
            return (status, Html(html_page)).into_response();
        }

        // For other errors (400, 401, 500), return plain text —
        // the frontend toast handler consumes this.
        (status, message).into_response()
    }
}
