# 列表-详情返回导航优化 — 设计文档

> 关联 Issue: [#62](https://github.com/swloki/abt2/issues/62)
> 状态: **已定稿**
> 方案: 中间件 URL 状态记忆
> 日期: 2026-06-15

## 一、问题陈述

系统中所有"列表页 → 详情页"跳转的业务模块（29 个详情页），返回按钮采用硬路由跳转到列表根路径，用户在列表页设置的筛选条件、翻页位置全部丢失。

期望：返回时精准还原用户进入详情前的列表状态（筛选参数 + 翻页）。

## 二、方案选型

评估了五个候选方案，最终选定 **中间件 URL 状态记忆**。

选定理由：
- **handler 零改动**：列表页/详情页 handler 均无需修改
- **对用户/开发者无感**：中间件自动拦截记录/恢复，不需要 `rem` 参数或任何标记
- **逻辑集中**：一个中间件文件，一处实现
- 复用现成 `tower-sessions`，技术模式与 `auth_middleware` 完全一致

## 三、核心机制

### 3.1 中间件拦截逻辑

注册一个全局中间件（在 `auth_middleware` 之后），对所有 GET 请求执行：

```
GET 请求进来：
├─ 有 query string
│   → 按 path 保存完整 query string 到 Session（HashMap<String, String>）
│   → 正常传递给 handler
│
├─ 无 query string + 该 path 在 Session 中有保存状态
│   → 将保存的 query 注入请求 URI，handler 直接处理带参请求（无重定向，单次请求）
│
└─ 无 query string + 无保存状态
    → 正常传递给 handler

### 3.2 记录触发

列表页筛选/翻页时，HTMX 发起 GET 请求（如 `/admin/md/bom?keyword=电源&page=5`），URL 自带 query string，中间件自动记录。**不需要 `hx-push-url`、不需要 `rem` 参数、不需要改表单**。

### 3.3 恢复触发

用户从详情页点击返回按钮（`<a href="/admin/md/bom">`，无 query），中间件检测到该 path 有保存状态，将保存的 query string 透明注入请求 URI（`/admin/md/bom?keyword=电源&page=5`），handler 的 `Query<T>` 提取器直接解析带参请求，渲染筛选结果。**无 HTTP 重定向，单次请求完成；返回按钮 href 不需要改**。

### 3.4 多级导航

仅带 query 的请求更新 Session（列表页筛选/翻页）。详情页 URL 如 `/admin/md/bom/123` 的 path 不同，不会覆盖列表 key。因此从 `列表 → 详情A → 详情B` 返回时，一步回到带筛选的列表。

### 3.5 不恢复的内容

- 页面滚动位置（翻页已通过 URL page 参数保留）
- 未提交的搜索框文字（HTMX 300ms 自动提交，窗口极短）

## 四、详细设计

### 4.1 Session 存储

```
Session key: "list_urls"
value: HashMap<String, String>  // {请求 path: query string（不含 ?）}

示例:
{
  "/admin/md/bom":       "keyword=电源&status=1&page=5",
  "/admin/wms/stock-in": "doc_number=RK&warehouse_id=3&page=2",
}
```

key 是 `request.uri().path()`（不含 query），value 是 `request.uri().query()`（不含 `?`）。

### 4.2 中间件实现

新建 `abt-web/src/middleware/list_state.rs`：

```rust
use std::collections::HashMap;

use axum::body::Body;
use axum::extract::Request;
use axum::http::Uri;
use axum::middleware::Next;
use axum::response::Response;
use tower_sessions::Session;

const LIST_URLS_KEY: &str = "list_urls";

fn should_skip(path: &str) -> bool {
    path.starts_with("/static")
        || path.starts_with("/favicon")
        || path == "/login"
        || path == "/logout"
}

pub async fn list_state_middleware(session: Session, request: Request<Body>, next: Next) -> Response {
    if request.method() != axum::http::Method::GET {
        return next.run(request).await;
    }

    let uri = request.uri().clone();
    let path = uri.path().to_string();

    if should_skip(&path) {
        return next.run(request).await;
    }

    // 情况1：有 query string → 记录最新状态
    if let Some(query) = uri.query() {
        let mut urls: HashMap<String, String> = session
            .get(LIST_URLS_KEY).await.ok().flatten().unwrap_or_default();
        urls.insert(path, query.to_string());
        if let Err(e) = session.insert(LIST_URLS_KEY, &urls).await {
            tracing::warn!("Failed to save list URL state: {e}");
        }
        return next.run(request).await;
    }

    // 情况2：无 query + 有保存状态 → 注入参数到请求 URI（不重定向）
    let saved_query = session
        .get::<HashMap<String, String>>(LIST_URLS_KEY).await.ok().flatten()
        .and_then(|urls| urls.get(&path).cloned());

    if let Some(query) = saved_query {
        let new_uri = format!("{path}?{query}");
        if let Ok(uri) = new_uri.parse::<Uri>() {
            let (mut parts, body) = request.into_parts();
            parts.uri = uri;
            return next.run(Request::from_parts(parts, body)).await;
        }
    }

    // 情况3：无 query + 无保存状态 → 正常
    next.run(request).await
}
```

### 4.3 中间件注册

在 `routes/mod.rs` 中，紧跟 `auth_middleware` 之后注册：

```rust
.layer(middleware::from_fn_with_state(
    state.clone(),
    auth_middleware,
))
.layer(middleware::from_fn(list_state_middleware))  // 新增
```

中间件执行顺序：`CompressionLayer → SessionManagerLayer → list_state → auth_middleware → router`。`list_state` 在 `auth_middleware` 之前执行（外层），但它不依赖 auth（只依赖 Session，已由 SessionManagerLayer 注入）。或者放在 auth_middleware 之后（内层），确保只处理已认证请求。

**推荐放在 auth_middleware 之后**（内层），避免对未认证请求（重定向到 /login）做无意义的 URL 记录。

### 4.4 RequestContext.original_uri 字段（移除）

原方案在 `RequestContext` 加 `original_uri` 字段——**中间件方案不需要**，因为中间件直接从 `request.uri()` 获取。handler 完全不参与。

## 五、改动范围

| 项目 | 文件数 | 改动 |
|---|---|---|
| 新建中间件 `list_state.rs` | 1 | ~50 行 |
| 中间件模块声明 `middleware/mod.rs` | 1 | 2 行 |
| 注册中间件 `routes/mod.rs` | 1 | +1 行 |
| **列表页 handler** | **0** | 零改动 |
| **详情页 handler** | **0** | 零改动 |
| **详情页模板/返回按钮** | **0** | 零改动 |
| **前端 JS / Hyperscript** | **0** | 零改动 |
| **筛选表单** | **0** | 零改动 |

**总改动：3 个文件，~55 行新增代码。**

## 六、边缘情况处理

| 场景 | 行为 | 评估 |
|---|---|---|
| 首次直接访问列表（Session 无记录） | 正常加载 | ✅ |
| 从详情返回列表 | 302 重定向到带参 URL | ✅ 核心场景 |
| 多级导航（列表→详情A→详情B）返回 | 一步回列表（带筛选） | ✅ |
| 翻页请求 `?page=3` | 有 query → 记录最新状态 | ✅ |
| 手动输入列表根路径想重置 | 恢复旧筛选 | ⚠️ 与"记忆"行为一致，大多数 ERP 习惯 |
| Session 过期 | 无保存状态，正常加载 | ✅ |
| 同用户多标签页 | Session 存最后操作的状态 | ✅ 合理 |
| 详情页带 query（如 `?tab=cost`） | 按 path 独立记录，不干扰列表 | ✅ |
| URL 参数已失效 | 服务器忽略无效参数，返回正常列表 | ✅ |
| 静态资源 / 登录页 | 跳过，不记录 | ✅ |
| 参数注入后 URL 一致性 | 浏览器地址栏可能仍显示无参 URL，但页面内容正确（单次请求，无重定向往返） | ✅ 可接受 |

## 七、验证策略

1. **`cargo clippy`** 编译验证
2. **端到端验证**（agent-browser）：
   - 列表页筛选 + 翻页 → 进入详情 → 点返回 → 验证筛选/翻页保留
   - 多级导航：列表 → 详情A → 详情B → 返回 → 一步回列表
   - 直接打开列表（首次） → 正常加载，无重定向
   - 翻页 → 返回 → 恢复正确页码

## 八、方案弃选记录

| 方案 | 弃选原因 |
|---|---|
| A: 纯 history.back | 依赖浏览器历史栈，多级导航逐级回退，遗漏 hx-push-url 即失效 |
| B: history.back + 滚动恢复 | 核心缺陷同 A |
| C: from 参数 | URL 污染，改动面大 |
| D: 客户端 sessionStorage | 引入客户端状态管理，偏离 SSR 架构 |
| E: 服务端 Session + handler 记录 | 需改 47 个列表页 handler + 29 个详情页 handler + 模板 |
| **F: 中间件拦截（选定）** | handler/模板/前端全零改动，仅 3 个文件 ~55 行 |
