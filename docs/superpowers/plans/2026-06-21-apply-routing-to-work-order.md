# 从工艺路径加载（选路径→列工序→设产出品+单价→应用）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把「从工艺路径加载」从自动加载改为：routing picker 选路径 → 抽屉列其工序、逐行设产出品+单价（预填模板值）→ 按 step_no 应用到工单工序。

**Architecture:** 新建 `routing_picker` 组件（镜像 `product_picker`，`/api/routings/search`）。Service 用 `apply_routing_to_work_order` 取代 `load_routings_from_template`。按钮开 picker → 选路径触发 GET apply-from-routing 抽屉（工序×产出品 picker+单价，预填 get_detail 值）→ POST 应用（JSON hidden 收集，仿 OM materials_json）。

**Tech Stack:** Rust 2024 / axum + TypedPath / sqlx / Maud / HTMX 2.0.10 / Hyperscript / async-trait / rust_decimal。

## Global Constraints

- **沟通用中文；commit message 中文**，结尾 `Co-Authored-By: Claude <noreply@anthropic.com>`
- **不要 `cargo run`**（服务在跑）；验证用 `cargo clippy` + `cargo test`
- **代码导航用 `lsp`**，禁文本搜索代替查定义/引用
- **跨模块只走 Service trait/Model**
- **错误禁止静默丢弃**：`?`/`map_err`；`DomainError::validation/business_rule/not_found`
- **所有 TypedPath**，禁硬编码 URL（API 搜索端点用 TypedPath）
- **样式 100% UnoCSS**，禁 `style=""` 内联
- **测试 DB 集成**，`abt-web/tests/`，**必须 `--test-threads=1`**
- ⚠️ **执行要点**：`cargo check -p abt-web --tests` ~13s；**必须真实跑 `cargo check`+`cargo test` 并贴输出**；**推荐 Inline 执行**

## 参考文件
- spec：`docs/superpowers/specs/2026-06-21-apply-routing-to-work-order-design.md`
- `abt-web/src/components/product_picker.rs`（picker + `/api/products/search` 范式；`router()` 在 `routes/mod.rs:150` merge）
- `abt-core/src/master_data/routing/service.rs`：`list(query)` / `get_detail(id)→RoutingDetail{steps:Vec<RoutingStep>}`
- `RoutingStep`（routing/model.rs:18）：`step_order`/`process_name`/`unit_price`/`product_id`
- `abt-web/src/pages/mes_order_detail.rs`：`tab_routing` 加载按钮(851)、`refresh_routing_tbody`、drawer 范式、`OrderRoutingLoadTemplatePath`
- `abt-web/src/pages/om_outsourcing_create.rs`：`materials_json`（JS 收集多行→hidden→serde_json 解析）范式
- `abt-core/src/mes/production_batch/{service.rs,implt.rs}`：`load_routings_from_template`（待移除）、`update_routing`（单事务范式）

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-web/src/components/routing_picker.rs` | routing 选择器 + `/api/routings/search` | 新建 |
| `abt-web/src/components/mod.rs` | 导出 routing_picker | 改 |
| `abt-web/src/routes/mod.rs` | merge routing_picker::router() | 改 |
| `abt-core/src/mes/production_batch/model.rs` | `RoutingStepApply` | 改 |
| `abt-core/src/mes/production_batch/service.rs`+`implt.rs` | 删 `load_routings_from_template`、加 `apply_routing_to_work_order` | 改 |
| `abt-web/src/routes/mes_order.rs` | 删 `OrderRoutingLoadTemplatePath`、加 `OrderRoutingApplyFromRoutingPath` | 改 |
| `abt-web/src/pages/mes_order_detail.rs` | 删 `load_routings_from_template` handler；加 get/post_apply_from_routing；按钮开 picker；抽屉壳；routing-selected 联动 | 改 |
| `abt-web/tests/mes_routing_price.rs` | 删 load_template 测试、加 apply 测试 | 改 |
| `docs/uml-design/04-mes.html` | trait 方法同步 | 改 |

---

### Task 1: routing_picker 组件 + 搜索端点

**Files:**
- Create: `abt-web/src/components/routing_picker.rs`
- Modify: `abt-web/src/components/mod.rs`（`pub mod routing_picker;`）
- Modify: `abt-web/src/routes/mod.rs`（`.merge(crate::components::routing_picker::router())`，紧随 product_picker merge 之后）

**Interfaces:**
- Consumes: `RoutingService::list(ctx, db, RoutingQuery{keyword}, page)`
- Produces: `routing_picker_modal(modal_id, target_id, display_id) -> Markup`；`GET /api/routings/search?keyword=` 端点

- [ ] **Step 1: 写组件**

新建 `abt-web/src/components/routing_picker.rs`（镜像 `product_picker.rs` 结构：TypedPath `/api/routings/search` + router + search handler + modal）。核心：

```rust
use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::RoutingQuery;
use abt_core::shared::types::PageParams;

use crate::errors::Result;
use crate::state::AppState;
use crate::utils::RequestContext;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/routings/search")]
pub struct RoutingSearchPath;

#[derive(Debug, Deserialize)]
pub struct RoutingSearchParams {
    pub keyword: Option<String>,
    pub target_id: Option<String>,
    pub display_id: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route(RoutingSearchPath::PATH, get(search_routings))
}

pub async fn search_routings(
    ctx: RequestContext,
    Query(params): Query<RoutingSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.routing_service();
    let result = svc.list(
        &service_ctx, &mut conn,
        RoutingQuery { keyword: params.keyword.filter(|s| !s.is_empty()) },
        PageParams::new(1, 20),
    ).await?;
    let target = params.target_id.as_deref().unwrap_or("routing_id");
    let display = params.display_id.as_deref().unwrap_or("routing-display");
    Ok(Html(routing_picker_results(&result.items, target, display).into_string()))
}

/// 工艺路径选择弹窗（fill-input 模式：选路径→填 hidden target_id + 显示名 + 发 routingSelected + 关弹窗）
pub fn routing_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    html! {
        div class="fixed inset-0 z-[1100] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            id=(modal_id) _=(close_hs) {
            div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                _="on click halt the event" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 class="text-lg font-semibold m-0" { "选择工艺路径" }
                    button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors" _=(close_hs) { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    div class="routing-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft" {
                        input type="hidden" name="target_id" value=(target_id);
                        input type="hidden" name="display_id" value=(display_id);
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "工艺路径名称" }
                            input class="routing-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text" name="keyword" placeholder="输入工艺路径名称…"
                                hx-get=(RoutingSearchPath::PATH) hx-trigger="keyup changed delay:300ms" hx-sync="this:replace"
                                hx-target="#routing-search-results" hx-swap="innerHTML" hx-include=".routing-search-bar" {}
                        }
                    }
                    div id="routing-search-results" class="max-h-[400px] overflow-y-auto"
                        hx-get=(RoutingSearchPath::PATH) hx-trigger="intersect once" hx-swap="innerHTML"
                        hx-vals=(format!("{{\"target_id\":\"{}\",\"display_id\":\"{}\"}}", target_id, display_id)) {
                        div class="flex items-center justify-center py-8 text-muted text-sm" { "加载中…" }
                    }
                }
            }
        }
    }
}

fn routing_picker_results(
    routings: &[abt_core::master_data::routing::model::Routing],
    target_id: &str, display_id: &str,
) -> Markup {
    // 注：点行 → 填 hidden target_id + 显示名到 display_id + 发 routingSelected + 关弹窗
    let click_hs = format!(
        "on click set #{}'s value to my @data-rid then put my @data-rname into #{} then remove .is-open from #routing-picker-modal then send routingSelected to body",
        target_id, display_id
    );
    html! {
        @if routings.is_empty() {
            div class="flex flex-col items-center justify-center py-12 text-muted" { p class="mt-2 text-sm" { "未找到匹配的工艺路径" } }
        } @else {
            div class="py-2" {
                @for r in routings {
                    div class="flex items-center justify-between p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                        data-rid=(r.id) data-rname=(r.name.as_str()) _=(click_hs.clone()) {
                        div class="text-sm font-medium text-fg" { (r.name.as_str()) }
                    }
                }
            }
        }
    }
}
```

> `Routing` 实体字段用 lsp 确认（`name` 等）；`modal_id` 在 click_hs 里硬编码 `#routing-picker-modal`——若需通用，把 modal_id 也 format 进 click_hs。本计划 modal 固定 id `routing-picker-modal`。

- [ ] **Step 2: 导出 + 注册**

`components/mod.rs` 加 `pub mod routing_picker;`。
`routes/mod.rs` 在 product_picker merge 后加 `.merge(crate::components::routing_picker::router())`。

- [ ] **Step 3: cargo check**

Run: `cargo check -p abt-web 2>&1 | grep -E "^error" | head`
Expected: 0 error（若 `Routing` 实体字段名/`RoutingQuery` 字段不符，lsp 调整）。

- [ ] **Step 4: 提交**

```bash
git add abt-web/src/components/routing_picker.rs abt-web/src/components/mod.rs abt-web/src/routes/mod.rs
git commit -m "feat(md): routing_picker 组件 + /api/routings/search（镜像 product_picker）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: service — 删 load_template、加 apply_routing_to_work_order

**Files:**
- Modify: `abt-core/src/mes/production_batch/model.rs`（加 `RoutingStepApply`）
- Modify: `abt-core/src/mes/production_batch/service.rs`（trait：删 `load_routings_from_template`、加 `apply_routing_to_work_order`）
- Modify: `abt-core/src/mes/production_batch/implt.rs`（删实现、加实现）

**Interfaces:**
- Produces: `RoutingStepApply { step_no, product_id: Option<i64>, unit_price: Decimal }`；`apply_routing_to_work_order(ctx, db, work_order_id, items) -> Result<usize>`

- [ ] **Step 1: 写失败测试**

`abt-web/tests/mes_routing_price.rs`：删除 `load_routings_from_template_no_panic` 测试，替换为：

```rust
#[tokio::test]
async fn apply_routing_to_work_order_applies_by_step_no() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "950").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    use abt_core::mes::production_batch::RoutingStepApply;
    let items = vec![
        RoutingStepApply { step_no: 1, product_id: Some(565), unit_price: Decimal::new(7, 0) },
        RoutingStepApply { step_no: 2, product_id: None, unit_price: Decimal::new(9, 0) },
        RoutingStepApply { step_no: 1, product_id: Some(565), unit_price: Decimal::ZERO }, // 单价≤0 → 跳过
    ];
    let n = svc.apply_routing_to_work_order(&ctx, &mut conn, wo_id, items).await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    let r1 = rs.iter().find(|r| r.step_no == 1).unwrap();
    let r2 = rs.iter().find(|r| r.step_no == 2).unwrap();
    assert_eq!(r1.product_id, Some(565));
    assert_eq!(r1.unit_price, Some(Decimal::new(7, 0)));
    assert_eq!(r2.unit_price, Some(Decimal::new(9, 0)));
    assert!(n >= 2, "应至少应用 2 行（step1+step2），实际 {n}");
}
```

- [ ] **Step 2: 跑确认失败**

Run: `cargo check -p abt-web --test mes_routing_price 2>&1 | grep -E "^error" | head`
Expected: `apply_routing_to_work_order`/`RoutingStepApply` 不存在。

- [ ] **Step 3: model + trait + impl**

`model.rs` 加：
```rust
/// 应用工艺路径配置到工单工序的单行（按 step_no 匹配）
pub struct RoutingStepApply {
    pub step_no: i32,
    pub product_id: Option<i64>,
    pub unit_price: rust_decimal::Decimal,
}
```
`service.rs` trait：删 `load_routings_from_template` 声明，加：
```rust
    /// 应用工艺路径的产出品+单价到工单工序（按 step_no，仅未报工+单价>0）。返回应用行数
    async fn apply_routing_to_work_order(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64, items: Vec<RoutingStepApply>,
    ) -> Result<usize>;
```
`implt.rs`：删 `load_routings_from_template` impl，加（单事务）：
```rust
    async fn apply_routing_to_work_order(
        &self, ctx: &ServiceContext, _db: PgExecutor<'_>,
        work_order_id: i64, items: Vec<RoutingStepApply>,
    ) -> Result<usize> {
        let mut conn = self.pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
        let wo = new_work_order_service(self.pool.clone()).find_by_id(ctx, &mut *conn, work_order_id).await?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许应用工艺路径"));
        }
        drop(conn);
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
        let mine = WorkOrderRoutingRepo::get_by_work_order_id(&mut *tx, work_order_id).await?;
        let mut applied = 0usize;
        for it in &items {
            if it.unit_price <= Decimal::ZERO { continue; }
            let Some(r) = mine.iter().find(|r| r.step_no == it.step_no) else { continue; };
            if WorkOrderRoutingRepo::has_report(&mut *tx, r.id).await? { continue; }
            sqlx::query(r#"UPDATE work_order_routings SET product_id = $2, unit_price = $3 WHERE id = $1"#)
                .bind(r.id).bind(it.product_id).bind(it.unit_price).execute(&mut *tx).await?;
            applied += 1;
        }
        if applied > 0 {
            new_audit_log_service(self.pool.clone())
                .record(ctx, &mut *tx, RecordAuditLogReq {
                    entity_type: "WorkOrder", entity_id: work_order_id,
                    action: AuditAction::Update,
                    changes: Some(json!(format!("应用工艺路径配置，{applied}行")),
                    context: None,
                }).await?;
        }
        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(applied)
    }
```
> `RoutingStepApply` import：`use super::model::RoutingStepApply;` 或 `use super::model::*;`（implt 已 `use super::model::*` 多半）。lsp 确认。

- [ ] **Step 4: cargo check + 测试**

Run: `cargo check -p abt-core 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- apply_routing --test-threads=1 2>&1 | tail -5`
Expected: 0 error，测试 PASS。

- [ ] **Step 5: 提交**

```bash
git add abt-core/src/mes/production_batch/model.rs abt-core/src/mes/production_batch/service.rs abt-core/src/mes/production_batch/implt.rs abt-web/tests/mes_routing_price.rs
git commit -m "refactor(mes): load_routings_from_template → apply_routing_to_work_order（按 step_no 应用产出品+单价）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: web — apply-from-routing 端点 + 按钮/picker/抽屉联动

**Files:**
- Modify: `abt-web/src/routes/mes_order.rs`（删 `OrderRoutingLoadTemplatePath` + 注册；加 `OrderRoutingApplyFromRoutingPath`）
- Modify: `abt-web/src/pages/mes_order_detail.rs`（删 `load_routings_from_template` handler + import；加 `get/post_apply_from_routing` + `ApplyForm`；按钮改开 picker；渲染 picker modal + apply 抽屉壳；routing-selected 联动）

**Interfaces:**
- Consumes: Task 1 `routing_picker_modal`；Task 2 `apply_routing_to_work_order`；`RoutingService::get_detail`
- Produces: `GET/POST /admin/mes/orders/{order_id}/routings/apply-from-routing`

- [ ] **Step 1: 路由**

`routes/mes_order.rs`：删 `OrderRoutingLoadTemplatePath` struct + `.route(...)`。加：
```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/apply-from-routing")]
pub struct OrderRoutingApplyFromRoutingPath { pub order_id: i64 }
```
router 注册：
```rust
        .route(OrderRoutingApplyFromRoutingPath::PATH, get(mes_order_detail::get_apply_from_routing).post(mes_order_detail::post_apply_from_routing))
```
（移除原 load-template 注册行）

- [ ] **Step 2: handler**

`mes_order_detail.rs` import：删 `OrderRoutingLoadTemplatePath`；加 `OrderRoutingApplyFromRoutingPath`；加 `use crate::components::routing_picker;`、`use abt_core::master_data::routing::RoutingService;`、`use abt_core::mes::production_batch::RoutingStepApply;`。

删 `load_routings_from_template` handler。加：
```rust
#[derive(Debug, serde::Deserialize)]
pub struct ApplyFromRoutingQuery {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub routing_id: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplyForm {
    pub apply_json: String, // 多行 [{step_no,product_id,unit_price}] 收集
}

/// GET：取选中路径的工序 → 抽屉表单（产出品 picker + 单价，预填模板值）
#[require_permission("WORK_ORDER", "update")]
pub async fn get_apply_from_routing(
    path: OrderRoutingApplyFromRoutingPath,
    ctx: RequestContext,
    axum::extract::Query(q): axum::extract::Query<ApplyFromRoutingQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let routing_id = q.routing_id.ok_or_else(|| abt_core::shared::types::DomainError::validation("请先选择工艺路径"))?;
    let detail = state.routing_service().get_detail(&service_ctx, &mut conn, routing_id).await?;
    Ok(Html(apply_from_routing_form(path.order_id, &detail.steps).into_string()))
}

/// POST：应用产出品+单价到工单工序（按 step_no）
#[require_permission("WORK_ORDER", "update")]
pub async fn post_apply_from_routing(
    path: OrderRoutingApplyFromRoutingPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ApplyForm>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let items: Vec<RoutingStepApply> = serde_json::from_str(&form.apply_json)
        .map_err(|e| abt_core::shared::types::DomainError::validation(format!("无效数据: {e}")))?;
    let svc = state.production_batch_service();
    svc.apply_routing_to_work_order(&service_ctx, &mut conn, path.order_id, items).await?;
    let body = refresh_routing_tbody(&state, &svc, &service_ctx, &mut conn, path.order_id).await?;
    Ok(Html(html! {
        (body)
        (maud::PreEscaped(r#"<script>document.querySelector('#routing-apply-drawer').classList.remove('open')</script>"#))
    }.into_string()))
}

/// 应用抽屉表单：每行工序 × (产出品 picker + 单价，预填)；JS 收集 → #apply-json hidden
fn apply_from_routing_form(work_order_id: i64, steps: &[abt_core::master_data::routing::model::RoutingStep]) -> Markup {
    html! {
        form id="routing-apply-form"
            hx-post=(OrderRoutingApplyFromRoutingPath { order_id: work_order_id }.to_string())
            hx-target="#routing-apply-drawer-body" hx-swap="innerHTML" {
            input type="hidden" name="apply_json" id="apply-json";
            div class="flex flex-col gap-3" {
                @for s in steps {
                    div class="border border-border-soft rounded-sm p-3" data-step-no=(s.step_order) {
                        div class="flex items-center justify-between mb-2" {
                            span class="text-sm font-medium" { (s.step_order) " - " (s.process_name.as_deref().unwrap_or(&s.process_code)) }
                        }
                        div class="grid grid-cols-2 gap-2" {
                            div {
                                span class="text-xs text-muted" { "产出品" }
                                div class="flex gap-1" {
                                    input type="hidden" class="ar-product-id" value=(s.product_id.map(|p| p.to_string()).unwrap_or_default());
                                    input type="text" class="ar-product-name flex-1 px-2 py-1 border border-border rounded-sm text-xs bg-surface" readonly
                                        value="" placeholder="选择产出品…";
                                    button type="button" class="text-xs px-2 py-1 border border-border rounded-sm cursor-pointer"
                                        _="on click add .is-open to #routing-apply-product-modal" { "选" }
                                }
                            }
                            div {
                                span class="text-xs text-muted" { "单价" }
                                input class="ar-unit-price w-full px-2 py-1 border border-border rounded-sm text-xs" type="number" step="any"
                                    value=(s.unit_price.map(|p| p.to_string()).unwrap_or_default());
                                input type="hidden" class="ar-step-no" value=(s.step_order);
                            }
                        }
                    }
                }
            }
        }
        // 收集脚本：每行 .ar-step-no/.ar-product-id/.ar-unit-price → JSON
        (maud::PreEscaped(r#"
        <script>
        function collectApplyJson() {
            var rows = document.querySelectorAll('#routing-apply-form [data-step-no]');
            var items = [];
            rows.forEach(function(row) {
                var stepNo = parseInt(row.querySelector('.ar-step-no').value);
                var pid = row.querySelector('.ar-product-id').value;
                var price = row.querySelector('.ar-unit-price').value;
                items.push({ step_no: stepNo, product_id: pid && pid !== '' ? Number(pid) : null, unit_price: parseFloat(price) || 0 });
            });
            document.querySelector('#apply-json').value = JSON.stringify(items);
        }
        document.querySelector('#routing-apply-form').addEventListener('submit', collectApplyJson);
        // product picker 选中 → 填到当前激活行（用 last-focused 记录）
        document.body.addEventListener('productSelected', function() {
            var pid = document.querySelector('#routing-apply-product-target')?.value;
            var pname = document.querySelector('#routing-apply-product-display')?.textContent;
            // 简化：填到第一个空 product-name 行；实现期可改为记录 active row
        });
        </script>
        "#))
        // 产出品选择弹窗（共用一个，target 固定）
        (crate::components::product_picker::product_picker_modal("routing-apply-product-modal", "routing-apply-product-target", "routing-apply-product-display"))
    }
}
```

> **产出品 picker 与多行的绑定**：多条工序共用一个 product_picker modal，需要知道"选中后填到哪一行"。实现期方案：每行「选」按钮点击时记录该行（`_="on click set $activeRow to closest [data-step-no]"`），productSelected 事件把 pid/pname 填到 `$activeRow` 的 `.ar-product-id`/`.ar-product-name`。**实现期把上面的 productSelected 监听改为基于 activeRow 填值**（用 hyperscript `set $activeRow` 或 JS 变量）。这是唯一需要实现期细化的交互点，spec 已允诺。

- [ ] **Step 3: 按钮改开 picker + 页面渲染 picker/抽屉壳**

`tab_routing` 的「从工艺路径加载」按钮：去掉 `hx-post`，改为开 picker：
```rust
 button type="button" class="inline-flex items-center gap-1 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
     _="on click add .is-open to #routing-picker-modal"
     title="选择一条工艺路径，加载其工序并设置产出品+单价" {
     (icon::download_icon("w-3.5 h-3.5"))
     "从工艺路径加载"
 }
```
`order_detail_page`（与编辑抽屉壳同处）渲染：
- routing picker modal：`(routing_picker::routing_picker_modal("routing-picker-modal", "routing-id-hidden", "routing-name-display"))`
- hidden `id="routing-id-hidden"` + 显示 span `id="routing-name-display"`
- routing-selected 联动：`hx-get=(OrderRoutingApplyFromRoutingPath{order_id: order.id}.to_string()) hx-trigger="routingSelected from:body" hx-target="#routing-apply-drawer-body" hx-include="#routing-id-hidden"` + `_="on 'htmx:afterRequest' add .open to #routing-apply-drawer"`（挂在 hidden input 或一个 sentinel 元素上）
- apply 抽屉壳：`(drawer::drawer("routing-apply-drawer","应用工艺路径","应用","routing-apply-form", html!{ div id="routing-apply-drawer-body" _="on htmx:afterSettle add .open to #routing-apply-drawer" {} }))`

> order_detail_page 需 `order.id` 构造 ApplyFromRoutingPath（已有 order 在 scope）。`tab_routing` 按钮不再需 hx-post，故移除其 `order_id` 依赖（但 tab_routing 仍保留 order_id 参数供「从最近工单加载」用）。

- [ ] **Step 4: cargo check + clippy + 测试**

Run: `cargo check -p abt-web 2>&1 | grep -E "^error" | head`
Run: `cargo clippy -p abt-core -p abt-web --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- --test-threads=1 2>&1 | tail -5`
Expected: 0 error，全 PASS。

- [ ] **Step 5: 提交**

```bash
git add abt-web/src/routes/mes_order.rs abt-web/src/pages/mes_order_detail.rs
git commit -m "feat(mes): 从工艺路径加载改为 选路径→列工序→设产出品+单价→应用

按钮开 routing picker；选路径触发 apply-from-routing 抽屉（工序×产出品picker+单价，
预填模板值）；POST apply_routing_to_work_order 按 step_no 应用。移除 load-template 端点。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: uml 同步 + 最终验证

**Files:**
- Modify: `docs/uml-design/04-mes.html`

- [ ] **Step 1: uml**

`ProductionBatchService`：`+load_routings_from_template(...)` → `+apply_routing_to_work_order(ctx, db, work_order_id, items) Result~usize~`

- [ ] **Step 2: 全量 clippy + 回归 serial**

Run: `cargo clippy --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price --test mes_batch --test mes_flow_e2e --test om_outsourcing_suggest -- --test-threads=1 2>&1 | grep -E "test result|FAILED"`
Expected: 0 error，全部 PASS。

- [ ] **Step 3: 提交**

```bash
git add docs/uml-design/04-mes.html
git commit -m "docs(uml): 同步 apply_routing_to_work_order

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec 覆盖**：§3.1 触发 → T3 Step3 按钮；§3.2 routing picker → T1；§3.3-3.4 apply 抽屉/POST → T3 Step2；§4.1 service → T2；§4.2 端点/移除 → T3 Step1；§6 错误（单价≤0 跳过）→ T2 impl；§7 测试 → T2 Step1；§8 uml → T4。全覆盖。

**2. 占位符扫描**：T3 Step2 的 product-picker-与多行绑定 标注「实现期改为 activeRow 填值」（给了 hyperscript `set $activeRow` 思路 + spec 已允诺）——这是唯一交互细化点，非空洞 TODO，给出了具体机制。其余代码完整。

**3. 类型一致性**：`apply_routing_to_work_order(ctx, db, wo_id, items: Vec<RoutingStepApply>) -> Result<usize>` 在 T2(trait+impl) 与 T3(handler 调用) 一致；`RoutingStepApply { step_no, product_id: Option, unit_price: Decimal }` T2 定义 + T3 测试一致；`OrderRoutingApplyFromRoutingPath { order_id }` T3 路由/handler/联动一致。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-apply-routing-to-work-order.md`. Two execution options:

**1. Subagent-Driven** — ⚠️ 上轮 sonnet 多次伪造验证，需 controller 交叉核对 + 自跑
**2. Inline Execution（推荐）** — 当前会话内联执行（cargo check 13s + 串行测试可靠）

Which approach?
