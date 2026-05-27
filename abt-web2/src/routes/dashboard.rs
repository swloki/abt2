use axum::Extension;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use crate::auth::session::Session;
use crate::layout::admin::admin_layout;
use crate::layout::base::base_html;

// ── Typed Path ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin")]
pub struct DashboardPath;

// ── Module Router ──

pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route(DashboardPath::PATH, get(get_dashboard))
        .route("/admin/", get(get_dashboard))
}

// ── Handler ──

pub async fn get_dashboard(
    _path: DashboardPath,
    Extension(session): Extension<Session>,
) -> axum::response::Html<String> {
    let content = dashboard_content(&session);
    let page = base_html("仪表盘", admin_layout(&session.claims, DashboardPath::PATH, content));
    axum::response::Html(page.into_string())
}

// ── Component ──

fn dashboard_content(session: &Session) -> Markup {
    html! {
        div {
            h1 class="page-title" { "仪表盘" }
            p class="mt-2 text-muted" {
                "欢迎回来, " (session.claims.display_name.as_str()) "!"
            }
            div class="mt-6 grid grid-cols-1 md:grid-cols-3 gap-4" {
                div class="stat-card" {
                    div class="stat-icon blue" {
                        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-5 h-5" {
                            path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4";
                        }
                    }
                    div {
                        div class="stat-value" { "--" }
                        div class="stat-label" { "产品总数" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" {
                        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-5 h-5" {
                            path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z";
                            line x1="12" y1="9" x2="12" y2="13";
                            line x1="12" y1="17" x2="12.01" y2="17";
                        }
                    }
                    div {
                        div class="stat-value" { "--" }
                        div class="stat-label" { "库存预警" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon blue" {
                        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-5 h-5" {
                            path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z";
                            polyline points="14 2 14 8 20 8";
                            line x1="16" y1="13" x2="8" y2="13";
                            line x1="16" y1="17" x2="8" y2="17";
                        }
                    }
                    div {
                        div class="stat-value" { "--" }
                        div class="stat-label" { "今日订单" }
                    }
                }
            }
        }
    }
}
