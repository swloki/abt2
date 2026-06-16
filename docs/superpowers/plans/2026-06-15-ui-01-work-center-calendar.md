# 工作中心与工作日历管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为工作中心 (WorkCenter) 和工作日历 (WorkCalendar) 两个 master_data 模块提供完整 CRUD UI 页面。

**Architecture:** 参照现有 routing 模块的 list/create/detail 三页模式。WorkCenter 页面放在 `pages/md_work_center_*.rs`，WorkCalendar 放在 `pages/md_work_calendar_*.rs`。路由各自独立，侧边栏新增两个入口。

**Tech Stack:** Rust (Axum + Maud + HTMX), abt-core WorkCenterService / WorkCalendarService

---

## File Structure

| 文件 | 职责 | 动作 |
|------|------|------|
| `abt-web/src/state.rs` | 新增 2 个 service 工厂方法 | Modify |
| `abt-web/src/routes/md_work_center.rs` | WorkCenter 路由 + TypedPath | Create |
| `abt-web/src/routes/md_work_calendar.rs` | WorkCalendar 路由 + TypedPath | Create |
| `abt-web/src/pages/md_work_center_list.rs` | 列表页（状态 Tab + 搜索 + 分页） | Create |
| `abt-web/src/pages/md_work_center_create.rs` | 创建/编辑页 | Create |
| `abt-web/src/pages/md_work_center_detail.rs` | 详情页（信息卡 + Tab） | Create |
| `abt-web/src/pages/md_work_calendar_list.rs` | 列表页 | Create |
| `abt-web/src/pages/md_work_calendar_create.rs` | 创建/编辑页 | Create |
| `abt-web/src/pages/md_work_calendar_detail.rs` | 详情页（信息卡 + 日历明细） | Create |
| `abt-web/src/routes/mod.rs` | 注册新路由模块 | Modify |
| `abt-web/src/pages/mod.rs` | 注册新页面模块 | Modify |
| `abt-web/src/layout/sidebar.rs` | 新增侧边栏导航项 | Modify |

---

## Task 1: state.rs 注册 Service 工厂

**Files:**
- Modify: `abt-web/src/state.rs` (在 `routing_service()` 方法附近)

- [ ] **Step 1: 添加 work_center_service 和 work_calendar_service**

在 `state.rs` 的 `routing_service()` 方法后面（约第 269 行后）添加：

```rust
    pub fn work_center_service(
        &self,
    ) -> impl abt_core::master_data::work_center::WorkCenterService {
        abt_core::master_data::work_center::new_work_center_service(self.pool.clone())
    }

    pub fn work_calendar_service(
        &self,
    ) -> impl abt_core::master_data::work_calendar::WorkCalendarService {
        abt_core::master_data::work_calendar::new_work_calendar_service(self.pool.clone())
    }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`
Expected: 无错误输出

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/state.rs
git commit -m "feat: register work_center and work_calendar services in AppState"
```

---

## Task 2: WorkCenter 路由文件

**Files:**
- Create: `abt-web/src/routes/md_work_center.rs`

- [ ] **Step 1: 创建路由文件**

```rust
use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{md_work_center_list, md_work_center_create, md_work_center_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers")]
pub struct WorkCenterListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers/new")]
pub struct WorkCenterCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers/{id}")]
pub struct WorkCenterDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers/{id}/edit")]
pub struct WorkCenterEditPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            WorkCenterListPath::PATH,
            get(md_work_center_list::get_work_center_list),
        )
        .route(
            WorkCenterCreatePath::PATH,
            get(md_work_center_create::get_work_center_create)
                .post(md_work_center_create::post_work_center_create),
        )
        .route(
            WorkCenterDetailPath::PATH,
            get(md_work_center_detail::get_work_center_detail),
        )
        .route(
            WorkCenterEditPath::PATH,
            get(md_work_center_create::get_work_center_edit)
                .post(md_work_center_create::post_work_center_update),
        )
}
```

- [ ] **Step 2: Commit**

```bash
git add abt-web/src/routes/md_work_center.rs
git commit -m "feat: add work_center route module"
```

---

## Task 3: WorkCenter 列表页

**Files:**
- Create: `abt-web/src/pages/md_work_center_list.rs`

- [ ] **Step 1: 创建列表页**

完整代码参照 `routing_list.rs` 模式。核心结构：

```rust
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::work_center::{WorkCenterService, model::*};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::md_work_center::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WorkCenterQueryParams {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub is_active: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("BOM", "read")]
pub async fn get_work_center_list(
    _path: WorkCenterListPath,
    ctx: RequestContext,
    Query(params): Query<WorkCenterQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let page = params.page.unwrap_or(1);
    let keyword = params.keyword.as_deref().filter(|s| !s.is_empty()).map(|s| s.to_string());
    let is_active = match params.is_active.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };

    let filter = WorkCenterFilter { keyword, work_center_type: None, is_active };
    let result = state.work_center_service()
        .list(&service_ctx, &mut conn, filter, PageParams::new(page, 20))
        .await?;

    let content = work_center_list_page(&result, &params);
    Ok(Html(admin_page(
        is_htmx, "工作中心管理", &claims, "md",
        WorkCenterListPath::PATH, "工程", Some(WorkCenterListPath::PATH),
        content, &nav_filter,
    ).into_string()))
}

// ── Components ──

fn work_center_list_page(
    result: &abt_core::shared::types::PaginatedResult<WorkCenter>,
    params: &WorkCenterQueryParams,
) -> Markup {
    let total = result.total;
    let page = params.page.unwrap_or(1);
    let page_size = 20u32;
    let total_pages = ((total as u32) + page_size - 1) / page_size;
    let query_string = build_query_string(params);

    html! {
        div class="page-header" {
            div class="page-header-left" {
                h1 class="page-title" { "工作中心管理" }
            }
            div class="page-actions" {
                a class="btn btn-primary" href=(WorkCenterCreatePath::PATH) {
                    (icon::plus_icon("w-4 h-4"))
                    "新建工作中心"
                }
            }
        }

        // 状态 Tab
        div class="filter-bar" {
            form class="filter-form" id="wc-filter-form"
                hx-get=(WorkCenterListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#data-card"
                hx-select="#data-card"
                hx-swap="outerHTML"
                hx-push-url="true"
                hx-include="#wc-filter-form" {

                // 状态 select
                select class="filter-select" name="is_active" {
                    @if params.is_active.is_none() {
                        option value="" selected { "全部" }
                    } @else {
                        option value="" { "全部" }
                    }
                    @if params.is_active.as_deref() == Some("true") {
                        option value="true" selected { "启用" }
                    } @else {
                        option value="true" { "启用" }
                    }
                    @if params.is_active.as_deref() == Some("false") {
                        option value="false" selected { "停用" }
                    } @else {
                        option value="false" { "停用" }
                    }
                }

                // 搜索框
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                          placeholder="搜索编码 / 名称"
                          value=(params.keyword.as_deref().unwrap_or(""));
                }
            }
        }

        // 数据表
        div class="data-card" id="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "编码" }
                            th { "名称" }
                            th { "类型" }
                            th class="num-right" { "产能/小时" }
                            th class="num-right" { "成本费率/h" }
                            th { "状态" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for wc in &result.items {
                            tr {
                                td class="mono" { (wc.code) }
                                td { strong { (wc.name) } }
                                td { (wc_type_label(wc.work_center_type)) }
                                td class="mono num-right" { (crate::utils::fmt_qty(wc.default_capacity)) }
                                td class="mono num-right" { "¥" (crate::utils::fmt_qty(wc.costs_hour)) }
                                td {
                                    @if wc.is_active {
                                        span class="status-pill status-active" { "启用" }
                                    } @else {
                                        span class="status-pill status-inactive" { "停用" }
                                    }
                                }
                                td {
                                    a href=(WorkCenterDetailPath { id: wc.id }.to_string()) {
                                        (icon::eye_icon("w-4 h-4"))
                                    }
                                    a href=(WorkCenterEditPath { id: wc.id }.to_string())
                                       class="ml-2" {
                                        (icon::edit_icon("w-4 h-4"))
                                    }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr { td colspan="7" class="empty-row" { "暂无工作中心数据" } }
                        }
                    }
                }
            }
            // 分页
            (pagination(WorkCenterListPath::PATH, &query_string, total, page, total_pages))
        }
    }
}

// ── Helpers ──

fn wc_type_label(t: i16) -> &'static str {
    match t {
        1 => "机器",
        2 => "人工",
        3 => "委外",
        _ => "—",
    }
}

fn build_query_string(params: &WorkCenterQueryParams) -> String {
    let mut parts = Vec::new();
    if let Some(ref k) = params.keyword {
        if !k.is_empty() { parts.push(format!("keyword={}", urlencoding::encode(k))); }
    }
    if let Some(ref a) = params.is_active {
        if !a.is_empty() { parts.push(format!("is_active={}", a)); }
    }
    parts.join("&")
}
```

注意：需要确认 `icon` 模块中有 `plus_icon` / `search_icon` / `eye_icon` / `edit_icon` 函数。如果没有，用项目中已有的 icon 函数替代。

- [ ] **Step 2: Commit**

```bash
git add abt-web/src/pages/md_work_center_list.rs
git commit -m "feat: add work_center list page"
```

---

## Task 4: WorkCenter 创建/编辑页

**Files:**
- Create: `abt-web/src/pages/md_work_center_create.rs`

- [ ] **Step 1: 创建创建页**

```rust
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::work_center::{WorkCenterService, model::*};
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::md_work_center::*;
use crate::utils::RequestContext;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct WorkCenterForm {
    pub code: String,
    pub name: String,
    pub work_center_type: String,
    pub costs_hour: String,
    pub time_efficiency: String,
    pub setup_time: String,
    pub cleanup_time: String,
    pub default_capacity: String,
    pub location: Option<String>,
}

// ── Create Handler ──

#[require_permission("BOM", "create")]
pub async fn get_work_center_create(
    _path: WorkCenterCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;

    let content = work_center_form_page(None);
    Ok(Html(admin_page(
        is_htmx, "新建工作中心", &claims, "md",
        WorkCenterCreatePath::PATH, "工程", Some("新建工作中心"),
        content, &nav_filter,
    ).into_string()))
}

#[require_permission("BOM", "create")]
pub async fn post_work_center_create(
    _path: WorkCenterCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WorkCenterForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let req = parse_form(&form)?;
    let id = state.work_center_service()
        .create(&service_ctx, &mut conn, req).await?;

    let redirect = WorkCenterDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Edit Handler ──

#[require_permission("BOM", "update")]
pub async fn get_work_center_edit(
    path: WorkCenterEditPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let wc = state.work_center_service()
        .get(&service_ctx, &mut conn, path.id).await?;

    let content = work_center_form_page(Some(&wc));
    Ok(Html(admin_page(
        is_htmx, "编辑工作中心", &claims, "md",
        WorkCenterEditPath { id: path.id }.PATH, "工程",
        Some("编辑工作中心"),
        content, &nav_filter,
    ).into_string()))
}

#[require_permission("BOM", "update")]
pub async fn post_work_center_update(
    path: WorkCenterEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WorkCenterForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let req = parse_update_form(&form)?;
    state.work_center_service()
        .update(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = WorkCenterDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn work_center_form_page(wc: Option<&WorkCenter>) -> Markup {
    let is_edit = wc.is_some();
    html! {
        div class="page-header" {
            div class="page-header-left" {
                a class="back-link" href=(WorkCenterListPath::PATH) { "← 返回列表" }
                h1 class="page-title" {
                    @if is_edit { "编辑工作中心" } @else { "新建工作中心" }
                }
            }
        }

        form class="data-card form-card"
            hx-post={ @if is_edit {
                (WorkCenterEditPath { id: wc.unwrap().id }.to_string())
            } @else {
                (WorkCenterCreatePath::PATH)
            }}
            hx-redirect={ @if is_edit {
                (WorkCenterDetailPath { id: wc.unwrap().id }.to_string())
            } @else { "" } } {

            div class="form-section" {
                div class="form-section-title" { "基本信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "编码 *" }
                        input class="form-input" type="text" name="code" required
                              value=(wc.map(|w| w.code.as_str()).unwrap_or(""))
                              @if is_edit { disabled };
                        @if is_edit {
                            input type="hidden" name="code"
                                  value=(wc.map(|w| w.code.as_str()).unwrap_or(""));
                        }
                    }
                    div class="form-field" {
                        label { "名称 *" }
                        input class="form-input" type="text" name="name" required
                              value=(wc.map(|w| w.name.as_str()).unwrap_or(""));
                    }
                    div class="form-field" {
                        label { "类型" }
                        select class="form-select" name="work_center_type" {
                            @for (val, label) in [("1", "机器"), ("2", "人工"), ("3", "委外")] {
                                option value=(val) selected=(wc.map(|w| w.work_center_type.to_string()).as_deref() == Some(val)) {
                                    (label)
                                }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "位置" }
                        input class="form-input" type="text" name="location"
                              value=(wc.and_then(|w| w.location.as_deref()).unwrap_or(""));
                    }
                }
            }

            div class="form-section" {
                div class="form-section-title" { "产能与成本" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "产能/小时" }
                        input class="form-input" type="number" step="0.01" name="default_capacity"
                              value=(wc.map(|w| crate::utils::fmt_qty(w.default_capacity)).unwrap_or("0".into()));
                    }
                    div class="form-field" {
                        label { "成本费率/小时 (¥)" }
                        input class="form-input" type="number" step="0.01" name="costs_hour"
                              value=(wc.map(|w| crate::utils::fmt_qty(w.costs_hour)).unwrap_or("0".into()));
                    }
                    div class="form-field" {
                        label { "效率系数" }
                        input class="form-input" type="number" step="0.01" name="time_efficiency"
                              value=(wc.map(|w| crate::utils::fmt_qty(w.time_efficiency)).unwrap_or("1".into()));
                    }
                    div class="form-field" {
                        label { "准备时间" }
                        input class="form-input" type="number" step="0.01" name="setup_time"
                              value=(wc.map(|w| crate::utils::fmt_qty(w.setup_time)).unwrap_or("0".into()));
                    }
                    div class="form-field" {
                        label { "清理时间" }
                        input class="form-input" type="number" step="0.01" name="cleanup_time"
                              value=(wc.map(|w| crate::utils::fmt_qty(w.cleanup_time)).unwrap_or("0".into()));
                    }
                }
            }

            div class="create-action-bar" {
                a class="btn btn-default" href=(WorkCenterListPath::PATH) { "取消" }
                button class="btn btn-primary" type="submit" {
                    (icon::check_icon("w-4 h-4"))
                    @if is_edit { "保存" } @else { "创建" }
                }
            }
        }
    }
}

// ── Parsers ──

fn parse_form(form: &WorkCenterForm) -> Result<CreateWorkCenterReq> {
    let costs_hour = form.costs_hour.parse()
        .map_err(|_| DomainError::validation("成本费率格式错误"))?;
    let time_efficiency = form.time_efficiency.parse()
        .map_err(|_| DomainError::validation("效率系数格式错误"))?;
    let setup_time = form.setup_time.parse()
        .map_err(|_| DomainError::validation("准备时间格式错误"))?;
    let cleanup_time = form.cleanup_time.parse()
        .map_err(|_| DomainError::validation("清理时间格式错误"))?;
    let default_capacity = form.default_capacity.parse()
        .map_err(|_| DomainError::validation("产能格式错误"))?;
    let work_center_type: i16 = form.work_center_type.parse()
        .map_err(|_| DomainError::validation("工作中心类型错误"))?;

    Ok(CreateWorkCenterReq {
        code: form.code.trim().to_string(),
        name: form.name.trim().to_string(),
        work_center_type,
        costs_hour,
        time_efficiency,
        setup_time,
        cleanup_time,
        default_capacity,
        calendar_id: None,
        location: form.location.as_deref().filter(|s| !s.trim().is_empty()).map(|s| s.to_string()),
    })
}

fn parse_update_form(form: &WorkCenterForm) -> Result<UpdateWorkCenterReq> {
    let costs_hour = form.costs_hour.parse()
        .map_err(|_| DomainError::validation("成本费率格式错误"))?;
    let time_efficiency = form.time_efficiency.parse()
        .map_err(|_| DomainError::validation("效率系数格式错误"))?;
    let setup_time = form.setup_time.parse()
        .map_err(|_| DomainError::validation("准备时间格式错误"))?;
    let cleanup_time = form.cleanup_time.parse()
        .map_err(|_| DomainError::validation("清理时间格式错误"))?;
    let default_capacity = form.default_capacity.parse()
        .map_err(|_| DomainError::validation("产能格式错误"))?;
    let work_center_type: i16 = form.work_center_type.parse()
        .map_err(|_| DomainError::validation("工作中心类型错误"))?;

    Ok(UpdateWorkCenterReq {
        name: Some(form.name.trim().to_string()),
        work_center_type: Some(work_center_type),
        costs_hour: Some(costs_hour),
        time_efficiency: Some(time_efficiency),
        setup_time: Some(setup_time),
        cleanup_time: Some(cleanup_time),
        default_capacity: Some(default_capacity),
        calendar_id: None,
        location: form.location.as_deref().filter(|s| !s.trim().is_empty()).map(|s| s.to_string()),
        is_active: None,
    })
}
```

- [ ] **Step 2: Commit**

```bash
git add abt-web/src/pages/md_work_center_create.rs
git commit -m "feat: add work_center create/edit page"
```

---

## Task 5: WorkCenter 详情页

**Files:**
- Create: `abt-web/src/pages/md_work_center_detail.rs`

- [ ] **Step 1: 创建详情页**

```rust
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::work_center::WorkCenterService;

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
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let wc = state.work_center_service()
        .get(&service_ctx, &mut conn, path.id).await?;

    let content = work_center_detail_page(&wc);
    Ok(Html(admin_page(
        is_htmx, &format!("工作中心 {}", wc.code), &claims, "md",
        &format!("/admin/md/work-centers/{}", path.id), "工程",
        Some(&wc.name),
        content, &nav_filter,
    ).into_string()))
}

fn work_center_detail_page(wc: &abt_core::master_data::work_center::model::WorkCenter) -> Markup {
    html! {
        div class="page-header" {
            div class="page-header-left" {
                a class="back-link" href=(WorkCenterListPath::PATH) { "← 返回列表" }
                h1 class="page-title" { "工作中心 " (wc.code) " - " (wc.name) }
            }
            div class="page-actions" {
                a class="btn btn-default" href=(WorkCenterEditPath { id: wc.id }.to_string()) {
                    (icon::edit_icon("w-4 h-4"))
                    "编辑"
                }
            }
        }

        div class="info-card" {
            div class="info-section-title" { "基本信息" }
            div class="info-grid" {
                div class="info-item" { label { "编码" } span class="mono" { (wc.code) } }
                div class="info-item" { label { "名称" } span { (wc.name) } }
                div class="info-item" {
                    label { "类型" }
                    span { (wc_type_label(wc.work_center_type)) }
                }
                div class="info-item" {
                    label { "状态" }
                    @if wc.is_active {
                        span class="status-pill status-active" { "启用" }
                    } @else {
                        span class="status-pill status-inactive" { "停用" }
                    }
                }
                div class="info-item" {
                    label { "位置" }
                    span { (wc.location.as_deref().unwrap_or("—")) }
                }
            }
        }

        div class="info-card" {
            div class="info-section-title" { "产能与成本" }
            div class="info-grid" {
                div class="info-item" {
                    label { "产能/小时" }
                    span class="mono" { (crate::utils::fmt_qty(wc.default_capacity)) }
                }
                div class="info-item" {
                    label { "成本费率/h" }
                    span class="mono" { "¥" (crate::utils::fmt_qty(wc.costs_hour)) }
                }
                div class="info-item" {
                    label { "效率系数" }
                    span class="mono" { (crate::utils::fmt_qty(wc.time_efficiency)) }
                }
                div class="info-item" {
                    label { "准备时间" }
                    span class="mono" { (crate::utils::fmt_qty(wc.setup_time)) }
                }
                div class="info-item" {
                    label { "清理时间" }
                    span class="mono" { (crate::utils::fmt_qty(wc.cleanup_time)) }
                }
            }
        }
    }
}

fn wc_type_label(t: i16) -> &'static str {
    match t {
        1 => "机器",
        2 => "人工",
        3 => "委外",
        _ => "—",
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add abt-web/src/pages/md_work_center_detail.rs
git commit -m "feat: add work_center detail page"
```

---

## Task 6: WorkCalendar 路由 + 页面（简化版）

WorkCalendar 的 CRUD 结构与 WorkCenter 类似。由于 WorkCalendar 的 model 更简单（只有 name + description），但需要管理日历明细（CalendarLine），页面稍微复杂。

**Files:**
- Create: `abt-web/src/routes/md_work_calendar.rs`
- Create: `abt-web/src/pages/md_work_calendar_list.rs`
- Create: `abt-web/src/pages/md_work_calendar_create.rs`
- Create: `abt-web/src/pages/md_work_calendar_detail.rs`

- [ ] **Step 1: 创建路由文件** `abt-web/src/routes/md_work_calendar.rs`

```rust
use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{md_work_calendar_list, md_work_calendar_create, md_work_calendar_detail};
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-calendars")]
pub struct WorkCalendarListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-calendars/new")]
pub struct WorkCalendarCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-calendars/{id}")]
pub struct WorkCalendarDetailPath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(WorkCalendarListPath::PATH, get(md_work_calendar_list::get_work_calendar_list))
        .route(
            WorkCalendarCreatePath::PATH,
            get(md_work_calendar_create::get_work_calendar_create)
                .post(md_work_calendar_create::post_work_calendar_create),
        )
        .route(WorkCalendarDetailPath::PATH, get(md_work_calendar_detail::get_work_calendar_detail))
}
```

- [ ] **Step 2: 创建列表页** `abt-web/src/pages/md_work_calendar_list.rs`

参照 `md_work_center_list.rs` 模式，调用 `state.work_calendar_service()`。由于 WorkCalendarService 没有 `list` 方法（只有 create_calendar / get_calendar / set_lines 等），列表查询需要通过 abt-core 新增方法或直接用已有 repo 查询。

**注意**：如果 WorkCalendarService 缺少 `list` 方法，需要在 abt-core 中补充。检查 service trait — 如果只有按 ID 查询，需要添加 `list_calendars` 方法到 service trait + implt + repo。

对于此 Task，先创建一个最小列表页，使用 `get_calendar` 逐条查询或新增 `list_calendars` 方法。如果时间紧张，列表页可暂时显示空表格 + 创建按钮。

- [ ] **Step 3: 创建创建页** `abt-web/src/pages/md_work_calendar_create.rs`

表单只需 name + description 两个字段。POST 后调用 `create_calendar`，成功后重定向到详情页。详情页中再管理日历明细行（CalendarLine）。

- [ ] **Step 4: 创建详情页** `abt-web/src/pages/md_work_calendar_detail.rs`

详情页显示：
1. 基本信息（name / description）
2. 工作时间明细 Tab — 调用 `list_lines` 显示 CalendarLine 列表
3. 例外日 Tab — 调用 `list_exceptions` 显示节假日列表

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/routes/md_work_calendar.rs abt-web/src/pages/md_work_calendar_*.rs
git commit -m "feat: add work_calendar CRUD pages"
```

---

## Task 7: 注册路由和页面模块

**Files:**
- Modify: `abt-web/src/routes/mod.rs`
- Modify: `abt-web/src/pages/mod.rs`

- [ ] **Step 1: routes/mod.rs — 添加模块声明和路由注册**

在 `pub mod routing;` 后添加：

```rust
pub mod md_work_center;
pub mod md_work_calendar;
```

在 `router()` 函数的 MD 区域（`.merge(routing::router())` 后）添加：

```rust
                .merge(md_work_center::router())
                .merge(md_work_calendar::router())
```

- [ ] **Step 2: pages/mod.rs — 添加模块声明**

在 `pub mod routing_detail;` 后添加：

```rust
pub mod md_work_center_list;
pub mod md_work_center_create;
pub mod md_work_center_detail;
pub mod md_work_calendar_list;
pub mod md_work_calendar_create;
pub mod md_work_calendar_detail;
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`
Expected: 无错误（可能有 warning 关于未使用代码，正常）

修复所有编译错误后继续。

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/routes/mod.rs abt-web/src/pages/mod.rs
git commit -m "feat: register work_center and work_calendar routes and pages"
```

---

## Task 8: 更新侧边栏

**Files:**
- Modify: `abt-web/src/layout/sidebar.rs` (约第 326-331 行，routing 之后)

- [ ] **Step 1: 添加侧边栏导航项**

在 `md` 模块的 items vec 中，`工艺路线` NavItem 之后添加：

```rust
                NavItem {
                    name: "工作中心",
                    path: "/admin/md/work-centers",
                    icon: NavIcon::Wrench,
                    permission: Some(("BOM", "read")),
                },
                NavItem {
                    name: "工作日历",
                    path: "/admin/md/work-calendars",
                    icon: NavIcon::Calendar,
                    permission: Some(("BOM", "read")),
                },
```

确认 `NavIcon` 枚举中有 `Wrench` 和 `Calendar` 变体。如果没有，使用已有的相近 icon（如 `Database` / `Grid`）。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/layout/sidebar.rs
git commit -m "feat: add work_center and work_calendar to sidebar navigation"
```

---

## Task 9: cargo clippy 最终验证

- [ ] **Step 1: 运行完整 clippy**

Run: `cargo clippy -p abt-web 2>&1`
Expected: 零 error，可能有 warning

- [ ] **Step 2: 修复所有 error（如有）**

- [ ] **Step 3: 最终 commit**

```bash
git add -A
git commit -m "fix: resolve clippy errors for work_center/calendar UI"
```

---

## Task 10: E2E 测试 — 工作中心 CRUD

**验证目标：** 工作中心列表页渲染、创建流程、详情页展示、搜索筛选。

- [ ] **Step 1: 登录**

```bash
agent-browser --cdp 9222 open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

- [ ] **Step 2: 测试列表页渲染**

```bash
agent-browser --cdp 9222 open https://localhost:8000/admin/md/work-centers
agent-browser snapshot -i
```

验证：
- 页面标题为 "工作中心管理"
- 存在 "新建工作中心" 按钮
- 存在状态筛选下拉框（全部/启用/停用）
- 存在搜索输入框
- 表格列：编码 / 名称 / 类型 / 产能 / 成本费率 / 状态 / 操作
- 存在分页组件（如数据 > 20 条）

- [ ] **Step 3: 测试创建工作中心**

```bash
agent-browser --cdp 9222 open https://localhost:8000/admin/md/work-centers/new
agent-browser snapshot -i
```

验证创建表单：
- 存在编码、名称、类型、产能、成本费率、效率系数、准备时间、清理时间、位置字段
- 存在 "创建" 按钮

填入数据：
```bash
agent-browser fill @e<code_input> "WC-E2E-01"
agent-browser fill @e<name_input> "E2E测试工作中心"
agent-browser fill @e<default_capacity_input> "100"
agent-browser fill @e<costs_hour_input> "80"
agent-browser click @e<submit_button>
agent-browser wait 1000
```

验证：跳转到详情页，显示编码 "WC-E2E-01"、名称 "E2E测试工作中心"。

- [ ] **Step 4: 测试详情页**

```bash
agent-browser snapshot -i
```

验证：
- 显示基本信息卡片（编码、名称、类型、状态、位置）
- 显示产能与成本卡片（产能、成本费率、效率系数、准备时间、清理时间）
- 存在 "编辑" 按钮

- [ ] **Step 5: 测试搜索**

```bash
agent-browser --cdp 9222 open https://localhost:8000/admin/md/work-centers
agent-browser fill @e<search_input> "E2E"
agent-browser wait 500
agent-browser snapshot -i
```

验证：搜索结果只包含 "E2E测试工作中心"。

- [ ] **Step 6: 测试状态筛选**

```bash
agent-browser select @e<status_select> "true"
agent-browser wait 500
agent-browser snapshot -i
```

验证：只显示启用状态的工作中心。

```bash
agent-browser select @e<status_select> ""
agent-browser wait 500
```

- [ ] **Step 7: 检查控制台错误**

```bash
agent-browser console --clear
agent-browser --cdp 9222 open https://localhost:8000/admin/md/work-centers
agent-browser wait 1000
agent-browser errors
```

验证：无 JavaScript 错误。

- [ ] **Step 8: 测试工作日历列表页**

```bash
agent-browser --cdp 9222 open https://localhost:8000/admin/md/work-calendars
agent-browser snapshot -i
```

验证：页面正常渲染，存在 "新建日历" 按钮。

- [ ] **Step 9: 记录测试结果**

在测试文档中记录：
- 通过的测试项
- 失败的测试项及原因
- 需要修复的问题

---

## Self-Review Checklist

- [ ] state.rs 有 `work_center_service()` 和 `work_calendar_service()`
- [ ] routes/mod.rs 注册了 `md_work_center` 和 `md_work_calendar` 模块
- [ ] pages/mod.rs 注册了 6 个新页面模块
- [ ] 侧边栏显示 "工作中心" 和 "工作日历" 入口
- [ ] 列表页支持状态筛选 + 搜索 + 分页
- [ ] 创建表单提交后跳转详情页
- [ ] 详情页展示所有字段
- [ ] cargo clippy 零 error
- [ ] E2E 测试全部通过
