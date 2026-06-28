use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::work_center::{
 model::{work_center_type_label, WorkCenter},
 WorkCenterService,
};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::md_work_center::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("BOM", "read")]
pub async fn get_work_center_detail(
 path: WorkCenterDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 let wc = state
 .work_center_service()
 .get(&service_ctx, &mut conn, path.id)
 .await?;

 let content = work_center_detail_page(&wc);
 Ok(Html(
 admin_page(
 is_htmx,
 &format!("工作中心 {}", wc.code),
 &claims,
 "production",
 &format!("/admin/md/work-centers/{}", path.id),
 "工程",
 Some(&wc.name),
 content,
 &nav_filter,
 )
 .into_string(),
 ))
}

fn work_center_detail_page(wc: &WorkCenter) -> Markup {
 html! {
    div class="flex items-center justify-between mb-6" {
        div class="flex items-center justify-between mb-6" {
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(WorkCenterListPath::PATH)
            { "← 返回列表" }
            h1 class="text-xl font-bold text-fg tracking-tight" { "工作中心 " (wc.code) " - " (wc.name) }
        }
        div class="flex gap-3" {
            a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                href=(WorkCenterEditPath { id: wc.id }.to_string())
            { (icon::edit_icon("w-4 h-4")) "编辑" }
        }
    }

    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
        div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "基本信息" }
        div class="grid gap-4" {
            div class="flex flex-col gap-1" {
                label { "编码" }
                span class="font-mono tabular-nums" { (wc.code) }
            }
            div class="flex flex-col gap-1" {
                label { "名称" }
                span { (wc.name) }
            }
            div class="flex flex-col gap-1" {
                label { "类型" }
                span { (work_center_type_label(wc.work_center_type)) }
            }
            div class="flex flex-col gap-1" {
                label { "状态" }
                @if wc.is_active {
                    span
                        class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-success-bg text-success"
                    { "启用" }
                } @else {
                    span
                        class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-danger-bg text-danger"
                    { "停用" }
                }
            }
            div class="flex flex-col gap-1" {
                label { "位置" }
                span { (wc.location.as_deref().unwrap_or("—")) }
            }
        }
    }

    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
        div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "产能与成本" }
        div class="grid gap-4" {
            div class="flex flex-col gap-1" {
                label { "产能/小时" }
                span class="font-mono tabular-nums" { (crate::utils::fmt_qty(wc.default_capacity)) }
            }
            div class="flex flex-col gap-1" {
                label { "成本费率/h" }
                span class="font-mono tabular-nums" { (crate::utils::fmt_amount(wc.costs_hour)) }
            }
            div class="flex flex-col gap-1" {
                label { "效率系数" }
                span class="font-mono tabular-nums" { (crate::utils::fmt_qty(wc.time_efficiency)) }
            }
            div class="flex flex-col gap-1" {
                label { "准备时间 (分钟)" }
                span class="font-mono tabular-nums" { (crate::utils::fmt_qty(wc.setup_time)) }
            }
            div class="flex flex-col gap-1" {
                label { "清理时间 (分钟)" }
                span class="font-mono tabular-nums" { (crate::utils::fmt_qty(wc.cleanup_time)) }
            }
        }
    }
}
}
