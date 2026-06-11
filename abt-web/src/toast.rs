use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::Form;
use maud::{html, Markup};
use dashmap::DashMap;
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use crate::auth::session::CURRENT_USER_KEY;
use abt_core::shared::identity::model::Claims;
use tower_sessions::Session;

// ── Model ──────────────────────────────────────────────

#[derive(Clone)]
pub struct ToastMessage {
    pub msg: String,
    pub r#type: ToastType,
    pub created_at: Instant,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ToastType {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastType {
    fn as_str(self) -> &'static str {
        match self {
            ToastType::Success => "success",
            ToastType::Error => "error",
            ToastType::Warning => "warning",
            ToastType::Info => "info",
        }
    }

    fn icon_svg(self) -> &'static str {
        match self {
            ToastType::Success => r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><path d="M22 11.08V12a10 10 0 11-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>"#,
            ToastType::Error => r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>"#,
            ToastType::Warning => r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>"#,
            ToastType::Info => r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>"#,
        }
    }
}

// ── Queue (DashMap) ────────────────────────────────────

static TOAST_QUEUE: LazyLock<DashMap<i64, Vec<ToastMessage>>> = LazyLock::new(DashMap::new);

const MAX_TOASTS_PER_USER: usize = 10;
const TOAST_TTL: Duration = Duration::from_secs(60);

/// 向用户队列追加一条 Toast 消息（原子操作，无竞态）
pub fn add_toast(user_id: i64, msg: impl Into<String>, r#type: ToastType) {
    let mut queue = TOAST_QUEUE.entry(user_id).or_default();
    if queue.len() >= MAX_TOASTS_PER_USER {
        queue.remove(0);
    }
    queue.push(ToastMessage {
        msg: msg.into(),
        r#type,
        created_at: Instant::now(),
    });
}

/// 写入 Toast + 设置 HX-Trigger 的便捷函数
/// 适用于 hx-swap="none" 的场景
pub fn toast_response(user_id: i64, msg: impl Into<String>, r#type: ToastType) -> Response {
    add_toast(user_id, msg, r#type);
    (
        StatusCode::OK,
        [("HX-Trigger", "showToast")],
    )
        .into_response()
}

// ── Client POST model ──────────────────────────────────

#[derive(Deserialize)]
pub struct ClientToastRequest {
    pub msg: String,
    pub r#type: String,
}

// ── Handler ────────────────────────────────────────────

/// POST /api/toast — 客户端直接提交消息，后端返回 Toast HTML
/// 用于客户端表单验证等不走业务 handler 的场景
pub async fn post_client_toast(Form(req): Form<ClientToastRequest>) -> Response {
    let toast_type = match req.r#type.as_str() {
        "error" => ToastType::Error,
        "warning" => ToastType::Warning,
        "info" => ToastType::Info,
        _ => ToastType::Success,
    };
    Html(
        render_toasts(&[ToastMessage {
            msg: req.msg,
            r#type: toast_type,
            created_at: Instant::now(),
        }])
        .into_string(),
    )
    .into_response()
}

/// GET /api/toast — 读后即焚，返回 Toast HTML
pub async fn get_toasts(session: Session) -> Response {
    let claims = session
        .get::<Claims>(CURRENT_USER_KEY)
        .await
        .ok()
        .flatten();

    let user_id = match claims {
        Some(c) => c.sub,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let messages = TOAST_QUEUE
        .remove(&user_id)
        .map(|(_, v)| v)
        .unwrap_or_default();

    let now = Instant::now();
    let fresh: Vec<_> = messages
        .into_iter()
        .filter(|m| now.duration_since(m.created_at) < TOAST_TTL)
        .collect();

    if fresh.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }

    Html(render_toasts(&fresh).into_string()).into_response()
}

// ── Rendering ──────────────────────────────────────────

fn render_single_toast(msg: &str, toast_type: ToastType) -> Markup {
    let type_str = toast_type.as_str();
    let icon = toast_type.icon_svg();
    html! {
        div class={"toast toast-" (type_str)} role="alert" {
            span class="toast-icon" { (maud::PreEscaped(icon)) }
            span class="toast-message" { (msg) }
            button class="toast-close" onclick="this.parentElement.remove()" { "×" }
        }
    }
}

fn render_toasts(messages: &[ToastMessage]) -> Markup {
    html! {
        div hx-swap-oob="innerHTML:.toast-container" {
            @for m in messages {
                (render_single_toast(&m.msg, m.r#type))
            }
        }
    }
}
