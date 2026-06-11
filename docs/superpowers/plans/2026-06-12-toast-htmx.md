# Toast Notification HTMX Component Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the JS-based toast notification with a server-driven HTMX component using DashMap queue.

**Architecture:** Handler calls `add_toast(user_id, msg, type)` to write to a process-global `DashMap` queue, then sets `HX-Trigger: showToast` response header. A hidden HTMX div in the layout listens for this event, fires `GET /api/toast` to consume the queue, and renders toast HTML via OOB swap. CSS animations handle lifecycle; `animationend` event cleans up DOM.

**Tech Stack:** Rust/Axum (handler + DashMap), Maud (HTML rendering), HTMX (event-driven reload), CSS animations, vanilla JS (error fallback + DOM cleanup only).

**Design Spec:** `docs/superpowers/specs/2026-06-12-toast-htmx-design.md`

---

### Task 1: Create toast module — model + queue + utility functions

**Files:**
- Create: `abt-web/src/toast.rs`
- Modify: `abt-web/src/main.rs` (add `mod toast;`)

- [ ] **Step 1: Create `abt-web/src/toast.rs` with model, queue, and utility functions**

```rust
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use maud::{html, Markup};
use dashmap::DashMap;
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
    let mut queue = TOAST_QUEUE.entry(user_id).or_insert_with(Vec::new);
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

// ── Handler ────────────────────────────────────────────

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
```

- [ ] **Step 2: Add `mod toast;` to `abt-web/src/main.rs`**

在现有 `mod` 声明区域（约第 1-11 行）添加一行：

```rust
mod toast;
```

- [ ] **Step 3: Run `cargo clippy` to verify compilation**

Run: `cargo clippy`
Expected: 0 errors (可能有 unused warnings，后续步骤会使用这些函数)

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/toast.rs abt-web/src/main.rs
git commit -m "feat(web): add toast module with DashMap queue and utilities (#9)"
```

---

### Task 2: Register GET /api/toast route

**Files:**
- Modify: `abt-web/src/routes/mod.rs`

- [ ] **Step 1: Add toast route to the router**

在 `routes/mod.rs` 的 `router()` 函数中，在 `auth::router()` 的 `.merge()` 之后、其他路由之前，添加 toast 路由：

找到类似这样的代码：
```rust
pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::router())
        .merge(
            dashboard::router()
            // ...
        )
```

在 `auth::router()` 的 merge 之后添加：
```rust
        .route("/api/toast", get(crate::toast::get_toasts))
```

- [ ] **Step 2: Verify the route compiles**

Run: `cargo clippy`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/routes/mod.rs
git commit -m "feat(web): register GET /api/toast route (#9)"
```

---

### Task 3: Update layout — toast_container as HTMX component

**Files:**
- Modify: `abt-web/src/layout/page.rs` (替换 `toast_container()` 函数，约第 96-99 行)

- [ ] **Step 1: Replace `toast_container()` with HTMX component**

将：
```rust
fn toast_container() -> Markup {
    html! {
        div class="toast-container" {}
    }
}
```

替换为：
```rust
fn toast_container() -> Markup {
    html! {
        div class="toast-container" {}
        div hx-get="/api/toast"
            hx-trigger="showToast from:body"
            hx-target=".toast-container"
            hx-swap="innerHTML"
            style="display:none" {}
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo clippy`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/layout/page.rs
git commit -m "feat(web): upgrade toast_container to HTMX component (#9)"
```

---

### Task 4: Refactor toast CSS — animation lifecycle + Flex stacking

**Files:**
- Modify: `static/base.css` (toast 相关样式，约第 2294-2319 行)

- [ ] **Step 1: Replace all toast CSS with new animation-based styles**

将 `.toast-container` 之后的所有 `.toast*` 相关样式替换为：

```css
/* Toast Notification — HTMX Component */
.toast-container {
  position: fixed;
  top: 20px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  pointer-events: none;
}

.toast {
  position: relative;
  pointer-events: auto;
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(10px);
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 12px;
  padding: 16px 24px;
  display: flex;
  align-items: center;
  gap: 12px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
  max-width: 400px;
  width: 90%;
  animation: toast-lifecycle 4s ease forwards;
}

@keyframes toast-lifecycle {
  0%   { opacity: 0; transform: translateY(-20px); }
  8%   { opacity: 1; transform: translateY(0); }
  85%  { opacity: 1; transform: translateY(0); }
  100% { opacity: 0; transform: translateY(-20px); }
}

.toast-icon { width: 20px; height: 20px; flex-shrink: 0; }
.toast-message { flex: 1; line-height: 1.5; }

.toast-close {
  background: rgba(0, 0, 0, 0.05);
  border: none;
  border-radius: 8px;
  padding: 4px 8px;
  cursor: pointer;
  font-size: 16px;
  color: rgba(0, 0, 0, 0.4);
  transition: all 0.15s ease;
}
.toast-close:hover { opacity: 1; background: rgba(255, 255, 255, 0.15); }

.toast-error {
  border-color: rgba(220, 38, 38, 0.2);
  background: rgba(254, 248, 248, 0.95);
  color: #991b1b;
}
.toast-error .toast-icon { color: #dc2626; }

.toast-success {
  border-color: rgba(16, 185, 129, 0.2);
  background: rgba(240, 253, 244, 0.95);
  color: #047857;
}
.toast-success .toast-icon { color: #10b981; }

.toast-warning {
  border-color: rgba(217, 119, 6, 0.2);
  background: rgba(254, 249, 195, 0.95);
  color: #92400e;
}
.toast-warning .toast-icon { color: #d97706; }

.toast-info {
  border-color: rgba(37, 99, 235, 0.2);
  background: rgba(239, 246, 255, 0.95);
  color: #1e40af;
}
.toast-info .toast-icon { color: #2563eb; }
```

注意删除以下不再需要的旧样式：
- `.toast.toast-show` （动画由 CSS keyframes 驱动，不再需要 `toast-show` class）
- 旧的 `.toast` 中的 `position: fixed`、`top`、`left`、`transform`、`transition` 属性
- 旧的 `z-index` 属性（移到了 `.toast-container`）

- [ ] **Step 2: Commit**

```bash
git add static/base.css
git commit -m "feat(css): refactor toast styles to CSS animation lifecycle (#9)"
```

---

### Task 5: Update app.js — remove showToast + add error fallback + DOM cleanup

**Files:**
- Modify: `static/app.js`

- [ ] **Step 1: Remove `window.showToast` function**

删除 `window.showToast = function (message, type) { ... }` 整个函数定义（约第 30-61 行）。

- [ ] **Step 2: Replace `htmx:afterRequest` handler with error fallback**

将现有的 `htmx:afterRequest` 监听器（约第 63-77 行）：
```javascript
document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;

    if (xhr.status === 401) {
        window.location.href = '/login';
        return;
    }

    var msg = (xhr.responseText || '').trim() || '操作失败';
    window.showToast(msg, 'error');
});
```

替换为：
```javascript
// 错误兜底：直接创建 error toast（绕过 DashMap 队列，因为 handler 已失败）
document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;

    if (xhr.status === 401) {
        window.location.href = '/login';
        return;
    }

    var msg = (xhr.responseText || '').trim() || '操作失败';
    var container = document.querySelector('.toast-container');
    if (!container) return;

    var div = document.createElement('div');
    div.className = 'toast toast-error';
    div.setAttribute('role', 'alert');
    div.innerHTML = '<span class="toast-icon"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg></span>' +
        '<span class="toast-message">' + msg.replace(/</g, '&lt;') + '</span>' +
        '<button class="toast-close" onclick="this.parentElement.remove()">×</button>';
    container.appendChild(div);
});
```

- [ ] **Step 3: Add `animationend` DOM cleanup listener**

在文件末尾（或在 `htmx:afterRequest` 监听器之后）添加：
```javascript
// CSS 动画结束后自动移除 DOM 节点，防止长时间使用后堆积透明元素
document.addEventListener('animationend', function (e) {
    if (e.target.classList.contains('toast')) {
        e.target.remove();
    }
});
```

- [ ] **Step 4: Search for any remaining `showToast` references and remove/update**

搜索整个 `static/` 目录中所有对 `window.showToast` 或 `showToast(` 的调用：
- 如果是成功提示（如 `exportDone` 事件监听器中的 `showToast('导出完成', 'success')`），替换为 `htmx.trigger(document.body, 'showToast')` — 前提是后端 handler 已经通过 `add_toast` 写入了消息
- 如果后端 handler 尚未迁移，暂时保留 `showToast` 调用（渐进式迁移）

- [ ] **Step 5: Commit**

```bash
git add static/app.js
git commit -m "feat(js): remove showToast, add error fallback and animationend cleanup (#9)"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run cargo clippy on the full project**

Run: `cargo clippy`
Expected: 0 errors, 0 warnings related to toast code

- [ ] **Step 2: Verify no remaining references to old showToast pattern**

Search `abt-web/src/` for any handler that uses `HX-Trigger` with custom event names that trigger `showToast` in JS — these handlers should be migrated to use `add_toast()` + `HX-Trigger: showToast` pattern. Document which handlers still need migration.

- [ ] **Step 3: Commit final state if any fixes were needed**

```bash
git add -A
git commit -m "chore: toast component implementation complete (#9)"
```

---

## Migration Guide (for existing handlers)

新代码使用以下模式：

```rust
// 模式 1：表单提交成功（hx-swap="none"）
async fn create_something(ctx: RequestContext, ...) -> WebResult<Response> {
    service.create(...).await?;
    Ok(toast_response(ctx.claims.sub, "创建成功", ToastType::Success))
}

// 模式 2：操作 + 返回 HTML
async fn delete_something(ctx: RequestContext, ...) -> WebResult<Markup> {
    service.delete(...).await?;
    crate::toast::add_toast(ctx.claims.sub, "删除成功", ToastType::Success);
    // 渲染页面 HTML 时在响应头带上 HX-Trigger
    Ok(html! { ... })
    // 注意：如果 handler 返回 WebResult<Markup>，需要改为返回 WebResult<Response>
    // 以便同时设置响应头和 HTML body
}
```

旧 handler 无需立即迁移，两种模式可并存。
