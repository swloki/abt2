use abt_core::shared::identity::model::Claims;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use crate::routes::dashboard::DashboardPath;
use crate::routes::login::LogoutPath;

pub fn header(claims: &Claims, current_path: &str) -> Markup {
    let display_name = if claims.display_name.is_empty() {
        &claims.username
    } else {
        &claims.display_name
    };
    html! {
        header class="flex h-14 items-center justify-between border-b border-slate-200/60 bg-white px-4 md:px-6" {
            div class="flex items-center gap-3" {
                div class="hidden md:flex items-center gap-2 text-sm text-slate-400" {
                    a href=(DashboardPath::PATH) class="transition-colors hover:text-slate-600" { "首页" }
                    span { "/" }
                    span class="font-medium text-slate-700" { (page_title(current_path)) }
                }
            }
            div class="flex items-center gap-3" {
                span class="text-sm text-slate-600" { (display_name) }
                form method="POST" action=(LogoutPath::PATH) {
                    button type="submit" class="text-sm text-slate-400 hover:text-slate-600" { "退出" }
                }
            }
        }
    }
}

fn page_title(path: &str) -> &str {
    match path {
        p if p == DashboardPath::PATH => "仪表盘",
        _ => "管理后台",
    }
}
