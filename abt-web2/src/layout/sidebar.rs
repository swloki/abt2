use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use crate::routes::dashboard::DashboardPath;

pub fn sidebar(current_path: &str) -> Markup {
    let items = menu_items();
    html! {
        aside class="hidden md:flex w-56 flex-col border-r border-slate-200/60 bg-white h-screen" {
            div class="flex h-14 items-center px-4 border-b border-slate-200/60" {
                span class="text-lg font-bold text-primary-600" { "ABT" }
                span class="ml-1 text-xs text-slate-400" { "管理系统" }
            }
            nav class="flex-1 overflow-y-auto px-2 py-3" {
                @for item in &items {
                    (menu_item(item, current_path))
                }
            }
        }
    }
}

struct MenuItem {
    name: &'static str,
    path: &'static str,
}

fn menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem { name: "仪表盘", path: DashboardPath::PATH },
    ]
}

fn menu_item(item: &MenuItem, current_path: &str) -> Markup {
    let active = current_path == item.path;
    html! {
        a href=(item.path)
           class=(if active { "flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium bg-primary-50 text-primary-700" }
                  else { "flex items-center gap-3 rounded-lg px-3 py-2 text-sm text-slate-600 hover:bg-slate-50 hover:text-slate-900" })
        {
            span class="text-base" {}
            (item.name)
        }
    }
}
