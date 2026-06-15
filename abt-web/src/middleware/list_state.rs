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
        || path == "/login"
        || path == "/logout"
}

/// 列表-详情返回导航状态记忆中间件。
///
/// 对所有 GET 请求：
/// - 有 query string → 按 path 保存 query 到 Session，正常处理
/// - 无 query + 该 path 有保存状态 → 将保存的 query 注入请求 URI，正常处理（无重定向）
/// - 无 query + 无保存状态 → 正常处理
///
/// 这样用户从详情页返回列表（URL 为列表根路径，无 query）时，
/// 中间件透明地将上次保存的筛选/翻页参数注入请求，handler 直接渲染带参结果。
pub async fn list_state_middleware(session: Session, request: Request<Body>, next: Next) -> Response {
    // 只处理 GET 请求
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

    // 情况2：无 query + 有保存状态 → 注入参数到请求 URI（不重定向）
    let saved_query = session
        .get::<HashMap<String, String>>(LIST_URLS_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|urls| urls.get(&path).cloned());

    if let Some(query) = saved_query {
        let new_uri = format!("{path}?{query}");
        if let Ok(uri) = new_uri.parse::<Uri>() {
            let (mut parts, body) = request.into_parts();
            parts.uri = uri;
            return next.run(Request::from_parts(parts, body)).await;
        }
        // parse 失败则 fallthrough 正常处理
    }

    // 情况3：无 query + 无保存状态 → 正常
    next.run(request).await
}
