use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::work_calendar::{model::*, WorkCalendarService};
use abt_core::shared::types::DomainError;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::md_work_calendar::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct WorkCalendarForm {
    pub name: String,
    pub description: Option<String>,
}

// ── Create Handler ──

#[require_permission("BOM", "create")]
pub async fn get_work_calendar_create(
    _path: WorkCalendarCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;

    let content = work_calendar_form_page();
    Ok(Html(
        admin_page(
            is_htmx,
            "新建工作日历",
            &claims,
            "md",
            WorkCalendarCreatePath::PATH,
            "工程",
            Some("新建工作日历"),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

#[require_permission("BOM", "create")]
pub async fn post_work_calendar_create(
    _path: WorkCalendarCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WorkCalendarForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let name = form.name.trim().to_string();
    if name.is_empty() {
        return Err(DomainError::validation("日历名称不能为空").into());
    }

    let req = CreateCalendarReq {
        name,
        description: form
            .description
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string()),
    };
    let id = state
        .work_calendar_service()
        .create_calendar(&service_ctx, &mut conn, req)
        .await?;

    let redirect = WorkCalendarDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn work_calendar_form_page() -> Markup {
    html! {
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6-left" {
                a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(WorkCalendarListPath::PATH) { "← 返回列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建工作日历" }
            }
        }

        form class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] form-card"
            hx-post=(WorkCalendarCreatePath::PATH) {

            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "基本信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "名称 *" }
                        input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="name" required;
                    }
                    div class="form-field span-2" {
                        label { "描述" }
                        input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="description";
                    }
                }
            }

            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(WorkCalendarListPath::PATH) { "取消" }
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="submit" {
                    (icon::check_circle_icon("w-4 h-4"))
                    "创建"
                }
            }
        }
    }
}
