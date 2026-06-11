# Toast Notification as Global HTMX Component

> Issue: #9
> Date: 2026-06-12
> Status: Approved

## Problem

当前 Toast 提示依赖 `static/app.js` 中的 `showToast()` 函数，前端逻辑耦合过重。成功提示需要后端通过 `HX-Trigger` 触发自定义事件，前端 JS 监听后调用 `showToast()`；错误提示由 `htmx:afterRequest` 全局监听捕获。这种方式缺乏统一的收口，且不符合 HTMX 的 HTML-on-the-wire 理念。

## Solution

将 Toast 升级为独立的 HTMX 控件，挂载在全局 layout 中。后端通过进程内 `DashMap` 队列管理消息（按 user_id 隔离），通过 `HX-Trigger` 事件驱动 Toast 组件重载渲染。

## Architecture

```
Handler (业务逻辑)
  ① add_toast(user_id, msg, type) → 写入 DashMap 队列（原子操作）
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
  ① 从 DashMap 按 user_id 取出并移除消息
  ② 渲染 Toast HTML → OOB swap 到 .toast-container
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
    pub created_at: Instant,  // 用于 TTL 过期清理
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

### Toast Queue Storage

- 使用进程内 `DashMap<i64, Vec<ToastMessage>>`（key 为 user_id），替代 Session 存储
- **原因**：`tower_sessions` 的 read-modify-write 不保证原子性，并发请求会互相覆盖（经典 lost update）
- `DashMap` 的 entry API 提供原子性插入，彻底消除竞态
- 读后即焚：每次 GET `/api/toast` 读取后立即清空队列
- Toast 消息本身是临时性的，服务器重启丢失可接受
- **防内存泄漏**（双重防御）：
  - **TTL 过期**：`get_toasts` 过滤掉 `created_at` 超过 60s 的消息
  - **队列上限**：`add_toast` 时限制每用户最多 10 条，超出丢弃最早的

## API

### GET /api/toast

从 DashMap 队列读取并消费 Toast 消息，返回 Toast HTML。

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
use dashmap::DashMap;

/// 全局 Toast 队列，key 为 user_id
static TOAST_QUEUE: Lazy<DashMap<i64, Vec<ToastMessage>>> = Lazy::new(DashMap::new);

/// 向用户队列追加一条 Toast 消息（原子操作，无竞态）
/// 每用户上限 10 条，超出丢弃最早的
const MAX_TOASTS_PER_USER: usize = 10;

pub fn add_toast(user_id: i64, msg: impl Into<String>, r#type: ToastType) {
    let mut queue = TOAST_QUEUE
        .entry(user_id)
        .or_insert_with(Vec::new);
    if queue.len() >= MAX_TOASTS_PER_USER {
        queue.remove(0);
    }
    queue.push(ToastMessage { msg: msg.into(), r#type, created_at: Instant::now() });
}

/// 写入 Toast 消息 + 设置 HX-Trigger 响应头的便捷函数
/// 适用于 hx-swap="none" 的场景
pub fn toast_response(user_id: i64, msg: impl Into<String>, r#type: ToastType) -> Response {
    add_toast(user_id, msg, r#type);
    (
        StatusCode::OK,
        [("HX-Trigger", "showToast")],
    )
        .into_response()
}

/// GET /api/toast handler：读后即焚，过滤超过 60s 的过期消息
const TOAST_TTL: Duration = Duration::from_secs(60);

pub async fn get_toasts(session: Session) -> Response {
    let user_id = get_current_user_id(&session); // 从 session 中取 user_id
    let messages = TOAST_QUEUE.remove(&user_id).map(|(_, v)| v).unwrap_or_default();
    let now = Instant::now();
    let fresh: Vec<_> = messages
        .into_iter()
        .filter(|m| now.duration_since(m.created_at) < TOAST_TTL)
        .collect();
    if fresh.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }
    Html(render_toasts(&fresh)).into_response()
}
```

### Handler Usage Patterns

```rust
// 场景1：表单提交成功（hx-swap="none" 模式）
async fn create_order(ctx: RequestContext, Form(form): Form<CreateOrder>) -> Response {
    order_service.create(ctx, form).await?;
    toast_response(ctx.user_id(), "创建成功", ToastType::Success)
}

// 场景2：操作 + 返回 HTML（同时触发 Toast）
async fn delete_order(ctx: RequestContext, Path(id): Path<i64>) -> Response {
    order_service.delete(ctx, id).await?;
    add_toast(ctx.user_id(), "删除成功", ToastType::Success);
    (
        [("HX-Trigger", "showToast")],
        Html(render_order_list()),
    )
        .into_response()
}
```

### Error Handling

**两层机制**：

1. **业务流程**：Handler 主动调用 `add_toast(user_id, msg, type)` → DashMap 队列 + `HX-Trigger: showToast` → Toast 组件渲染
2. **异常兜底**：保留 `htmx:afterRequest` 全局监听，改为触发 HTMX 事件

```javascript
// static/app.js — 兜底逻辑
document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;
    if (xhr.status === 401) { window.location.href = '/login'; return; }
    // 触发 HTMX 事件，走统一 Toast 流程
    // WebError::into_response 已将错误消息写入 DashMap 队列
    htmx.trigger(document.body, 'showToast', {});
});
```

**异常兜底方案**：`WebError::into_response` 中对业务错误（4xx）也调用 `add_toast` 写入 DashMap 队列，使异常走同一流程。对于无法获取 user_id 的极端情况，Toast 组件返回空响应——可接受的行为。

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

### DOM 残留清理

纯 CSS 动画结束后元素仍残留在 DOM 中（`opacity: 0` 但节点未移除），长时间使用会堆积无用节点。通过事件委托一行清理：

```javascript
// static/app.js — CSS 动画结束后自动移除 DOM 节点
document.addEventListener('animationend', function (e) {
    if (e.target.classList.contains('toast')) {
        e.target.remove();
    }
});
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `abt-web/src/toast.rs` | **New** | `ToastMessage`, `ToastType`, `TOAST_QUEUE` (DashMap), `add_toast()`, `toast_response()`, `GET /api/toast` handler |
| `abt-web/src/layout/page.rs` | **Modify** | `toast_container()` 改为 HTMX 组件 |
| `abt-web/src/errors.rs` | **Modify** | `WebError::into_response` 中业务错误调用 `add_toast` 写入队列 |
| `abt-web/src/app.rs` | **Modify** | 注册 `GET /api/toast` 路由 |
| `static/base.css` | **Modify** | Toast 样式重构：Flex 堆叠 + CSS animation |
| `static/app.js` | **Modify** | 移除 `showToast()`，兜底逻辑改为 `htmx.trigger()`，新增 `animationend` DOM 清理 |

## Out of Scope

- ❌ HTMX 自定义扩展
- ❌ Toast 持久化（刷新页面消息丢失是合理的）
- ❌ 批量改造现有所有 Handler（渐进式迁移，新代码用新模式，旧代码按需迁移）
