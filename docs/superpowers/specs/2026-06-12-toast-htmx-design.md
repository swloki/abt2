# Toast Notification as Global HTMX Component

> Issue: #9
> Date: 2026-06-12
> Status: Approved

## Problem

当前 Toast 提示依赖 `static/app.js` 中的 `showToast()` 函数，前端逻辑耦合过重。成功提示需要后端通过 `HX-Trigger` 触发自定义事件，前端 JS 监听后调用 `showToast()`；错误提示由 `htmx:afterRequest` 全局监听捕获。这种方式缺乏统一的收口，且不符合 HTMX 的 HTML-on-the-wire 理念。

## Solution

将 Toast 升级为独立的 HTMX 控件，挂载在全局 layout 中。后端通过 Session 队列管理消息，通过 `HX-Trigger` 事件驱动 Toast 组件重载渲染。

## Architecture

```
Handler (业务逻辑)
  ① add_toast(session, msg, type) → 写入 Session 队列
  ② 响应头带 HX-Trigger: showToast
         │
         │ HTTP Response
         ▼
Layout: toast-container + 隐藏 HTMX 触发器
  hx-get="/api/toast" hx-trigger="showToast from:body"
         │
         │ GET /api/toast
         ▼
GET /api/toast (读后即焚)
  ① 从 Session 读取 toast_messages
  ② 清空 Session 队列
  ③ 渲染 Toast HTML → OOB swap 到 .toast-container
         │
         │ Toast HTML
         ▼
.toast-container
  CSS animation 自动 4s 入场→停留→退场
  关闭按钮: onclick="this.parentElement.remove()"

兜底: htmx:afterRequest → htmx.trigger(body, 'showToast')
```

## Data Model

### ToastMessage

```rust
// abt-web/src/toast.rs
#[derive(Serialize, Deserialize, Clone)]
pub struct ToastMessage {
    pub msg: String,
    pub r#type: ToastType,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ToastType {
    Success,
    Error,
    Warning,
    Info,
}
```

### Session Storage

- Session key: `"toast_messages"`，值为 `Vec<ToastMessage>`
- 使用现有的 `tower_sessions` 框架
- 读后即焚：每次 GET `/api/toast` 读取后立即清空队列

## API

### GET /api/toast

从 Session 读取并消费 Toast 消息队列，返回 Toast HTML。

- **响应**：`hx-swap-oob="innerHTML:.toast-container"` 的 Toast HTML 片段
- **队列为空时**：返回 `204 No Content`

**请求示例**：
```
GET /api/toast
```

**响应示例**（队列有 2 条消息时）：
```html
<div hx-swap-oob="innerHTML:.toast-container">
  <div class="toast toast-show toast-success" role="alert">
    <span class="toast-icon"><!-- SVG --></span>
    <span class="toast-message">创建成功</span>
    <button class="toast-close" onclick="this.parentElement.remove()">×</button>
  </div>
  <div class="toast toast-show toast-warning" role="alert">
    <span class="toast-icon"><!-- SVG --></span>
    <span class="toast-message">库存不足</span>
    <button class="toast-close" onclick="this.parentElement.remove()">×</button>
  </div>
</div>
```

## Backend Integration

### Utility Functions

```rust
/// 向 Session 队列追加一条 Toast 消息
pub async fn add_toast(session: &Session, msg: impl Into<String>, r#type: ToastType) {
    let mut messages = session
        .get::<Vec<ToastMessage>>("toast_messages")
        .await
        .unwrap_or_default()
        .unwrap_or_default();
    messages.push(ToastMessage { msg: msg.into(), r#type });
    session.set("toast_messages", messages).await;
}

/// 写入 Toast 消息 + 设置 HX-Trigger 响应头的便捷函数
/// 适用于 hx-swap="none" 的场景
pub async fn toast_response(
    session: &Session,
    msg: impl Into<String>,
    r#type: ToastType,
) -> Response {
    add_toast(session, msg, r#type).await;
    (
        StatusCode::OK,
        [("HX-Trigger", "showToast")],
    )
        .into_response()
}
```

### Handler Usage Patterns

```rust
// 场景1：表单提交成功（hx-swap="none" 模式）
async fn create_order(session: Session, Form(form): Form<CreateOrder>) -> Response {
    order_service.create(ctx, form).await?;
    toast_response(&session, "创建成功", ToastType::Success).await
}

// 场景2：操作 + 返回 HTML（同时触发 Toast）
async fn delete_order(session: Session, Path(id): Path<i64>) -> Response {
    order_service.delete(ctx, id).await?;
    add_toast(&session, "删除成功", ToastType::Success).await;
    (
        [("HX-Trigger", "showToast")],
        Html(render_order_list()),
    )
        .into_response()
}
```

### Error Handling

**两层机制**：

1. **业务流程**：Handler 主动调用 `add_toast` → Session 队列 + `HX-Trigger: showToast` → Toast 组件渲染
2. **异常兜底**：保留 `htmx:afterRequest` 全局监听，改为触发 HTMX 事件而非调用 `showToast()`

```javascript
// static/app.js — 兜底逻辑
document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;
    if (xhr.status === 401) { window.location.href = '/login'; return; }
    // 触发 HTMX 事件，走统一 Toast 流程
    // 但此时 Session 中没有消息，需要特殊处理（见下方）
    htmx.trigger(document.body, 'showToast', {});
});
```

**异常兜底的 Session 缺失问题**：`htmx:afterRequest` 触发时 Session 中没有预写入的消息。解决方案：后端 `WebError::into_response` 中对业务错误（4xx）也写入 Session 队列，使异常走同一流程。对于无法写入 Session 的情况（如 Session 不可用），Toast 组件返回空响应，不显示任何内容——这是可接受的行为。

## Layout Integration

```rust
// abt-web/src/layout/page.rs
fn toast_container() -> Markup {
    html! {
        div class="toast-container" {}
        // 隐藏的 HTMX 触发器，监听 body 上的 showToast 事件
        div hx-get="/api/toast"
            hx-trigger="showToast from:body"
            hx-target=".toast-container"
            hx-swap="innerHTML"
            style="display:none" {}
    }
}
```

## CSS & Animation

```css
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
    animation: toast-lifecycle 4s ease forwards;
}

@keyframes toast-lifecycle {
    0%   { opacity: 0; transform: translateY(-20px); }
    8%   { opacity: 1; transform: translateY(0); }
    85%  { opacity: 1; transform: translateY(0); }
    100% { opacity: 0; transform: translateY(-20px); }
}
```

- 4 秒总时长：~0.3s 入场 → ~3.1s 停留 → ~0.6s 退场
- `animation-fill-mode: forwards` 退场后保持 `opacity: 0`
- 多条消息纵向堆叠，间距 8px
- 零 JS 定时器，纯 CSS 驱动

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `abt-web/src/toast.rs` | **New** | `ToastMessage`, `ToastType`, `add_toast()`, `toast_response()`, `GET /api/toast` handler |
| `abt-web/src/layout/page.rs` | **Modify** | `toast_container()` 改为 HTMX 组件 |
| `abt-web/src/errors.rs` | **Modify** | `WebError::into_response` 中业务错误写入 Session 队列 |
| `abt-web/src/app.rs` | **Modify** | 注册 `GET /api/toast` 路由 |
| `static/base.css` | **Modify** | Toast 样式重构：Flex 堆叠 + CSS animation |
| `static/app.js` | **Modify** | 移除 `showToast()`，兜底逻辑改为 `htmx.trigger()` |

## Out of Scope

- ❌ HTMX 自定义扩展
- ❌ Toast 持久化（刷新页面消息丢失是合理的）
- ❌ 批量改造现有所有 Handler（渐进式迁移，新代码用新模式，旧代码按需迁移）
