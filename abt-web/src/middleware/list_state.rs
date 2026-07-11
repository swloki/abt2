use std::collections::HashMap;

use axum::body::Body;
use axum::extract::Request;
use axum::http::Uri;
use axum::middleware::Next;
use axum::response::Response;
use tower_sessions::Session;

/// Session key: 存储各列表页最近一次带参请求的 query string
const LIST_URLS_KEY: &str = "list_urls";

/// 跳过非业务页面（静态资源、登录/登出）
fn should_skip(path: &str) -> bool {
    path.starts_with("/static")
        || path.starts_with("/favicon")
        || path.starts_with("/uploads")
        || path == "/login"
        || path == "/logout"
}

/// 列表-详情返回导航状态记忆中间件。
///
/// - 带 `restore=true` → 恢复该 path 在 Session 中保存的 query（透明注入 URI）
/// - 有 query（非 restore）→ 按 path 保存 query 到 Session（覆盖式），正常处理
/// - 无 query → 删除该 path 的保存状态（用户主动放弃筛选），正常处理
///
/// 详情页/创建页的"返回列表"链接 href 需附加 `?restore=true`，
/// 中间件据此恢复用户上次的筛选/翻页状态。
/// 从侧边栏菜单进入（无参）则始终显示全新列表。
pub async fn list_state_middleware(session: Session, request: Request<Body>, next: Next) -> Response {
    if request.method() != axum::http::Method::GET {
        return next.run(request).await;
    }

    let uri = request.uri().clone();
    let path = uri.path().to_string();

    if should_skip(&path) {
        return next.run(request).await;
    }

    let query = uri.query().unwrap_or("");

    // 情况1：带 restore=true → 恢复保存的状态
    if query.contains("restore=true") {
        let saved_query = session
            .get::<HashMap<String, String>>(LIST_URLS_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|urls| urls.get(&path).cloned());
        if let Some(saved) = saved_query {
            let new_uri = format!("{path}?{saved}");
            if let Ok(uri) = new_uri.parse::<Uri>() {
                let (mut parts, body) = request.into_parts();
                parts.uri = uri;
                return next.run(Request::from_parts(parts, body)).await;
            }
        }
        return next.run(request).await;
    }

    // 情况2：有 query（非 restore）→ 记录最新状态，正常处理
    if !query.is_empty() {
        let mut urls: HashMap<String, String> = session
            .get(LIST_URLS_KEY)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        urls.insert(path, query.to_string());
        if let Err(e) = session.insert(LIST_URLS_KEY, &urls).await {
            tracing::warn!("Failed to save list URL state: {e}");
        }
        return next.run(request).await;
    }

    // 情况3：无 query（菜单/侧边栏进入）→ 删除该 path 的保存状态，正常处理
    // 用户主动进入全新列表 = 放弃之前的筛选，清除残留避免过期 restore
    if let Some(mut urls) = session
        .get::<HashMap<String, String>>(LIST_URLS_KEY)
        .await
        .ok()
        .flatten()
        && urls.remove(&path).is_some()
    {
        let _ = session.insert(LIST_URLS_KEY, &urls).await;
    }

    next.run(request).await
}
