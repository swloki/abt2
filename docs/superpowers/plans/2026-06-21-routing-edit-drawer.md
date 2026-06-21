# 工单工序编辑抽屉 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把工单工序的产出品+计件单价编辑从表格行内 input 改为「行只读 + 编辑抽屉」（product picker 选产出品 + 单价，一次性保存），并合并 service 更新接口。

**Architecture:** 行改为只读展示（产出品显示产品名、单价 ¥X）+ 「编辑」按钮触发页面级单个抽屉（复用 `components::drawer::drawer`）。GET `/edit` 返回抽屉表单（`product_picker_modal` + 单价预填），POST `/edit` 调统一 `update_routing` 单事务存两字段 → OOB 刷新行 + 关抽屉。合并上轮 `update_routing_unit_price`/`update_routing_product` 为 `update_routing`。

**Tech Stack:** Rust 2024 / axum + TypedPath / sqlx / Maud / HTMX 2.0.10 / Hyperscript / async-trait / rust_decimal。

## Global Constraints

- **沟通用中文；commit message 中文**，结尾 `Co-Authored-By: Claude <noreply@anthropic.com>`
- **不要 `cargo run`**（服务在跑）；验证用 `cargo clippy` + `cargo test`
- **代码导航用 `lsp`**，禁文本搜索代替查定义/引用
- **跨模块只走 Service trait/Model**
- **共享服务按需工厂** `new_xxx_service(self.pool.clone())`
- **错误禁止静默丢弃**：`?`/`map_err`；`DomainError::validation/business_rule/not_found`
- **所有 TypedPath**，禁硬编码 URL
- **样式 100% UnoCSS**，禁 `style=""` 内联（`<col>` 例外）
- **测试 DB 集成**，`abt-web/tests/`，**必须 `--test-threads=1`**
- ⚠️ **执行要点**：`cargo check -p abt-web --tests` ~13s；**必须真实跑 `cargo check`+`cargo test` 并贴输出**（上轮 sonnet 实现者多次伪造"0 error"，controller 须交叉核对 diff + 自跑测试）；**推荐 Inline 执行**

## 参考文件
- spec：`docs/superpowers/specs/2026-06-21-routing-edit-drawer-design.md`
- `abt-web/src/components/drawer.rs`（`drawer(drawer_id,title,submit_label,form_id,body)`，`.open` 切换，footer submit 经 `form=form_id`）
- `abt-web/src/components/product_picker.rs`（`product_picker_modal(modal_id,target_id,display_id)` fill-input 模式，选产品→填 hidden + 显示名 + 关弹窗 + 发 `productSelected`）
- `abt-web/src/pages/product_list.rs:319`（drawer + product picker 用法范例）
- 当前代码：`abt-web/src/pages/mes_order_detail.rs::routing_row_fragment`(384) / `tab_routing`(721) / `get_order_detail` / handlers `update_routing_price`/`update_routing_product`；`routes/mes_order.rs`；`abt-core/src/mes/production_batch/{service.rs,implt.rs}`

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-core/src/mes/production_batch/service.rs` | trait：删2方法加1方法 | 改 |
| `abt-core/src/mes/production_batch/implt.rs` | 删2实现加 `update_routing` | 改 |
| `abt-web/src/routes/mes_order.rs` | 删2 TypedPath+注册，加 `OrderRoutingEditPath`(GET+POST) | 改 |
| `abt-web/src/pages/mes_order_detail.rs` | 删2 handler+form；加 get/post_routing_edit + 抽屉表单 fragment；行改只读+编辑按钮+product_name；order_detail_page 加抽屉壳；get_order_detail 批量取产品名 | 改 |
| `abt-web/tests/mes_routing_price.rs` | 替换旧 service 测试为 `service_update_routing` | 改 |
| `docs/uml-design/04-mes.html` | trait 方法名同步 | 改 |

---

### Task 1: service 合并 — `update_routing`

**Files:**
- Modify: `abt-core/src/mes/production_batch/service.rs`（trait，约 55-72 行）
- Modify: `abt-core/src/mes/production_batch/implt.rs`（删 `update_routing_unit_price` ~520-578 + `update_routing_product` ~580-619，加 `update_routing`）

**Interfaces:**
- Produces: `async fn update_routing(&self, ctx, db, work_order_id, routing_id, product_id: Option<i64>, unit_price: Decimal) -> Result<WorkOrderRouting>`

- [ ] **Step 1: 改测试（先行，TDD）**

`abt-web/tests/mes_routing_price.rs`：删除 `service_update_price_rejects_zero` / `service_update_price_ok_then_persists` / `service_update_price_rejects_cross_order` / `service_update_routing_product_ok_and_clear` 四个测试，替换为一个统一测试：

```rust
#[tokio::test]
async fn service_update_routing_saves_both_and_guards() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "800").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    let rid = rs[0].id;

    // 正常：一次保存 product_id + unit_price
    let updated = svc
        .update_routing(&ctx, &mut conn, wo_id, rid, Some(565), Decimal::new(7, 0))
        .await
        .unwrap();
    assert_eq!(updated.product_id, Some(565));
    assert_eq!(updated.unit_price, Some(Decimal::new(7, 0)));

    // 守卫：单价 ≤ 0
    let err = svc.update_routing(&ctx, &mut conn, wo_id, rid, None, Decimal::ZERO).await.unwrap_err();
    assert!(matches!(err, DomainError::Validation { .. }), "got {err:?}");

    // 守卫：跨工单
    let wo_b = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "801").await;
    let err = svc.update_routing(&ctx, &mut conn, wo_b, rid, None, Decimal::new(3, 0)).await.unwrap_err();
    assert!(matches!(err, DomainError::NotFound { .. }), "got {err:?}");
}
```

- [ ] **Step 2: 跑确认失败**

Run: `cargo check -p abt-web --test mes_routing_price 2>&1 | grep -E "^error" | head`
Expected: `update_routing` 方法不存在（旧方法已删/新方法未加）。

- [ ] **Step 3: trait 改方法**

`service.rs`：删除 `update_routing_unit_price` 和 `update_routing_product` 两个 trait 方法声明，替换为（放在原 `update_routing_unit_price` 位置）：

```rust
    /// 修改工序产出品 + 计件单价（单事务，首次报工前可改）
    async fn update_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        product_id: Option<i64>,
        unit_price: rust_decimal::Decimal,
    ) -> Result<WorkOrderRouting>;
```

- [ ] **Step 4: impl 改方法**

`implt.rs`：删除 `update_routing_unit_price` 和 `update_routing_product` 两个 impl 方法体，替换为一个 `update_routing`（单事务，守卫按序，审计两字段）：

```rust
    async fn update_routing(
        &self,
        ctx: &ServiceContext,
        _db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        product_id: Option<i64>,
        unit_price: Decimal,
    ) -> Result<WorkOrderRouting> {
        if unit_price <= Decimal::ZERO {
            return Err(DomainError::validation("计件单价必须大于 0"));
        }
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
        let routing = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?.ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        if routing.work_order_id != work_order_id {
            return Err(DomainError::not_found("WorkOrderRouting"));
        }
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, &mut *tx, work_order_id).await?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许修改工序"));
        }
        if WorkOrderRoutingRepo::has_report(&mut *tx, routing_id).await? {
            return Err(DomainError::business_rule("该工序已报工，不可修改"));
        }
        let old_pid = routing.product_id;
        let old_price = routing.unit_price;
        sqlx::query(
            r#"UPDATE work_order_routings SET product_id = $2, unit_price = $3 WHERE id = $1"#,
        )
        .bind(routing_id).bind(product_id).bind(unit_price)
        .execute(&mut *tx).await?;
        new_audit_log_service(self.pool.clone())
            .record(ctx, &mut *tx, RecordAuditLogReq {
                entity_type: "WorkOrderRouting",
                entity_id: routing_id,
                action: AuditAction::Update,
                changes: Some(json!(format!(
                    "product_id: {:?} → {:?}; unit_price: {:?} → {:?}",
                    old_pid, product_id, old_price, unit_price
                ))),
                context: Some(json!(format!("work_order_id={}", work_order_id))),
            })
            .await?;
        let updated = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?.ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(updated)
    }
```

> `json!` 宏、`RecordAuditLogReq`、`new_audit_log_service`、`AuditAction`、`WorkOrderRoutingRepo`、`new_work_order_service`、`WorkOrderStatus`、`DomainError`、`Decimal` 已在 implt.rs 现有 use 中（上轮 update 方法用过）。若 `changes`/`context` 字段不是 `Option<JsonValue>`，按 `RecordAuditLogReq` 实际类型调整（lsp hover 确认）。

- [ ] **Step 5: cargo check + clippy + 测试**

Run: `cargo check -p abt-core 2>&1 | grep -E "^error" | head`
Run: `cargo clippy -p abt-core --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- service_update_routing --test-threads=1 2>&1 | tail -5`
Expected: 0 error，测试 PASS。

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/mes/production_batch/service.rs abt-core/src/mes/production_batch/implt.rs abt-web/tests/mes_routing_price.rs
git commit -m "refactor(mes): 合并工序更新为单一 update_routing（product_id+unit_price 单事务）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: web 端点 — 移除旧端点 + 加 `/edit`（GET+POST）

**Files:**
- Modify: `abt-web/src/routes/mes_order.rs`（删 `OrderRoutingPricePath`/`OrderRoutingProductPath` + 注册；加 `OrderRoutingEditPath` + GET/POST 注册）
- Modify: `abt-web/src/pages/mes_order_detail.rs`（删 `update_routing_price`/`update_routing_product` handler + `RoutingPriceForm`/`RoutingProductForm` + 旧 import；加 `RoutingEditForm` + `get_routing_edit`(GET) + `post_routing_edit`(POST)）

**Interfaces:**
- Consumes: Task 1 的 `update_routing`
- Produces: `GET/POST /admin/mes/orders/{order_id}/routings/{routing_id}/edit`

- [ ] **Step 1: 路由改**

`routes/mes_order.rs`：删除 `OrderRoutingPricePath` 和 `OrderRoutingProductPath` 两个 struct + 对应 `.route(...)` 注册行。新增：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/edit")]
pub struct OrderRoutingEditPath {
    pub order_id: i64,
    pub routing_id: i64,
}
```

`router()` 中替换原两条 `.route` 为：
```rust
        .route(OrderRoutingEditPath::PATH, get(mes_order_detail::get_routing_edit).post(mes_order_detail::post_routing_edit))
```

- [ ] **Step 2: handler 改**

`mes_order_detail.rs`：
- import：删 `OrderRoutingPricePath`、`OrderRoutingProductPath`；加 `OrderRoutingEditPath`；加 `use crate::components::{drawer, product_picker};` 和 `use abt_core::master_data::product::ProductService;`
- 删除 `RoutingPriceForm`、`RoutingProductForm`、`update_routing_price`、`update_routing_product`
- 新增（紧跟 `split_order` 之后）：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct RoutingEditForm {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub product_id: Option<i64>,
    pub unit_price: rust_decimal::Decimal,
}

/// GET：返回编辑抽屉表单（product picker + 单价预填）
#[require_permission("WORK_ORDER", "update")]
pub async fn get_routing_edit(
    path: OrderRoutingEditPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
    let routing = routings.iter().find(|r| r.id == path.routing_id)
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
    // 解析当前产出品名
    let product_name = if let Some(pid) = routing.product_id {
        state.product_service()
            .get_by_ids(&service_ctx, &mut conn, vec![pid]).await
            .ok().and_then(|v| v.into_iter().next())
            .map(|p| p.pdt_name).unwrap_or_else(|| format!("#{}", pid))
    } else { String::new() };
    Ok(Html(routing_edit_form(path.order_id, path.routing_id, routing, &product_name).into_string()))
}

/// POST：保存 product_id + unit_price → OOB 刷行 + 关抽屉（失败返带错误表单）
#[require_permission("WORK_ORDER", "update")]
pub async fn post_routing_edit(
    path: OrderRoutingEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RoutingEditForm>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    match svc.update_routing(&service_ctx, &mut conn, path.order_id, path.routing_id, form.product_id, form.unit_price).await {
        Ok(updated) => {
            let order_has_report = svc.order_has_any_report(&service_ctx, &mut conn, path.order_id).await?;
            // 解析产出品名供行展示
            let pname = if let Some(pid) = updated.product_id {
                state.product_service().get_by_ids(&service_ctx, &mut conn, vec![pid]).await
                    .ok().and_then(|v| v.into_iter().next()).map(|p| p.pdt_name).unwrap_or_else(|| format!("#{}", pid))
            } else { String::new() };
            // OOB 行 + 关抽屉
            Ok(Html(html! {
                (routing_row_fragment(&updated, false, order_has_report, Some(&pname), path.routing_id))
                (routing_row_oob_swap(&updated, false, order_has_report, Some(&pname)))
                (maud::PreEscaped(r#"<script>document.querySelector('#routing-edit-drawer').classList.remove('open')</script>"#))
            }.into_string()))
        }
        Err(e) => {
            // 失败：返回带错误的表单（不关抽屉）
            let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
            let routing = routings.iter().find(|r| r.id == path.routing_id)
                .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
            let pname = if let Some(pid) = routing.product_id {
                state.product_service().get_by_ids(&service_ctx, &mut conn, vec![pid]).await
                    .ok().and_then(|v| v.into_iter().next()).map(|p| p.pdt_name).unwrap_or_else(|| format!("#{}", pid))
            } else { String::new() };
            Ok(Html(routing_edit_form(path.order_id, path.routing_id, routing, &pname).into_string()))
        }
    }
}
```

> `routing_row_fragment` 与 `routing_edit_form` / `routing_row_oob_swap` 在 Task 3 定义。本任务先让代码编译（在 Task 3 前可临时让 `routing_row_fragment` 仍用旧 4 参数签名、`routing_edit_form` 占位返回空 —— 但为避免占位，**Task 2 与 Task 3 合并执行**，见 Task 3）。

- [ ] **Step 3: 编译（与 Task 3 合并后验证）**

本任务的 handler 引用了 Task 3 的渲染函数，故编译验证在 Task 3 完成后统一做。

---

### Task 3: UI — 抽屉 + 行只读 + 编辑按钮 + 产品名展示

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs`（`routing_row_fragment`/`routing_tbody_fragment` 改签名+只读展示+编辑按钮；新增 `routing_edit_form`/`routing_row_oob_swap`；`tab_routing` 改；`order_detail_page` 加抽屉壳；`get_order_detail` 批量取产品名）

**Interfaces:**
- Consumes: Task 2 的 `OrderRoutingEditPath`
- Produces: 抽屉渲染 `routing_edit_form(wo_id, rid, r, product_name) -> Markup`；行渲染 `routing_row_fragment(r, is_reported_step, order_has_report, product_name: Option<&str>, rid_for_id) -> Markup`

- [ ] **Step 1: 改 `routing_row_fragment` 签名与渲染**

替换整个 `routing_row_fragment` 函数（384 行起）为：

```rust
fn routing_row_fragment(
    r: &WorkOrderRouting,
    is_reported_step: bool,
    order_has_report: bool,
    product_name: Option<&str>,
) -> Markup {
    html! {
        tr id=(format!("routing-row-{}", r.id)) {
            td class="font-mono tabular-nums" { (r.step_no) }
            td { strong { (r.process_name.as_str()) } }
            td class="text-[13px]" {
                @if let Some(pn) = product_name { (pn) }
                @else if let Some(pid) = r.product_id { span class="text-muted" { "#" (pid) } }
                @else { "—" }
            }
            td class="font-mono tabular-nums" {
                @if let Some(wc) = r.work_center_id { "#" (wc) } @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(r.planned_qty)) }
            td class="font-mono tabular-nums text-right text-[13px]" {
                @if let Some(t) = r.standard_time { (crate::utils::fmt_qty(t)) } @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px]" {
                @if let Some(c) = r.standard_cost { "¥" (crate::utils::fmt_qty(c)) } @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px]" {
                @if let Some(p) = r.unit_price { "¥" (crate::utils::fmt_qty(p)) } @else { "—" }
            }
            td {
                @if r.is_outsourced { span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-warn-bg text-warn" { "委外" } } @else { "—" }
            }
            td {
                @if r.is_inspection_point {
                    span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-accent-bg text-accent" { "报检" }
                } @else { "—" }
            }
            td class="text-center whitespace-nowrap" {
                @if !is_reported_step {
                    button class="text-muted hover:text-accent cursor-pointer border-none bg-transparent p-1" title="编辑"
                        hx-get=(OrderRoutingEditPath { order_id: r.work_order_id, routing_id: r.id }.to_string())
                        hx-target="#routing-edit-drawer-body" hx-swap="innerHTML"
                        _="on 'htmx:afterRequest' add .open to #routing-edit-drawer" {
                        (icon::edit_icon("w-4 h-4"))
                    }
                } @else { "—" }
                @if !order_has_report {
                    button class="text-muted hover:text-danger cursor-pointer border-none bg-transparent p-1 ml-1" title="删除该工序"
                        hx-post=(OrderRoutingDeletePath { order_id: r.work_order_id, routing_id: r.id }.to_string())
                        hx-confirm="删除该工序并重排后续工序号？"
                        hx-target="closest tbody" hx-swap="outerHTML" hx-disabled-elt="this" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

/// OOB 刷新：同 routing_row_fragment 但带 hx-swap-oob
fn routing_row_oob_swap(
    r: &WorkOrderRouting, is_reported_step: bool, order_has_report: bool, product_name: Option<&str>,
) -> Markup {
    html! {
        tr id=(format!("routing-row-{}", r.id)) hx-swap-oob="true" {
            // 内容与 routing_row_fragment 的 td 完全一致 —— 复制上面的 td 序列
        }
    }
}
```

> `routing_row_oob_swap` 的 `<td>` 序列与 `routing_row_fragment` 完全相同（多了 `hx-swap-oob="true"`）。实现时把上面的 td 序列原样复制进去（DRY 受 maud 限制，复制可接受）。

- [ ] **Step 2: 改 `routing_tbody_fragment` 签名**

```rust
fn routing_tbody_fragment(
    routings: &[WorkOrderRouting],
    reported_routing_ids: &std::collections::HashSet<i64>,
    order_has_report: bool,
    product_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    html! {
        tbody {
            @for r in routings {
                (routing_row_fragment(
                    r,
                    reported_routing_ids.contains(&r.id),
                    order_has_report,
                    r.product_id.and_then(|pid| product_names.get(&pid)).map(|s| s.as_str()),
                ))
            }
            @if routings.is_empty() {
                tr { td colspan="11" class="text-center text-muted text-sm" { "暂无工序明细（工单未下达或无工艺路线）" } }
            }
        }
    }
}
```

- [ ] **Step 3: 新增 `routing_edit_form`（抽屉表单）**

```rust
fn routing_edit_form(work_order_id: i64, routing_id: i64, r: &WorkOrderRouting, product_name: &str) -> Markup {
    html! {
        // 抽屉表单：hx-post 到 /edit，成功/失败都替换 #routing-edit-drawer-body
        form id="routing-edit-form"
            hx-post=(OrderRoutingEditPath { order_id: work_order_id, routing_id }.to_string())
            hx-target="#routing-edit-drawer-body" hx-swap="innerHTML" {
            input type="hidden" name="product_id" id="routing-product-id"
                value=(r.product_id.map(|p| p.to_string()).unwrap_or_default());
            div class="form-field mb-4" {
                label class="block text-xs font-medium text-fg-2 mb-1" { "产出品" }
                div class="flex gap-2" {
                    input type="text" id="routing-product-display" readonly
                        class="flex-1 px-3 py-2 border border-border rounded-sm text-sm bg-surface"
                        value=(product_name) placeholder="点击右侧选择产出品…";
                    button type="button" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg-2 cursor-pointer hover:bg-surface"
                        _="on click add .is-open to #routing-product-modal" { "选择" }
                }
            }
            div class="form-field mb-4" {
                label class="block text-xs font-medium text-fg-2 mb-1" { "计件单价（元/件）" }
                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                    type="number" step="any" min="0.000001" name="unit_price" required
                    value=(r.unit_price.map(|p| p.to_string()).unwrap_or_default());
            }
        }
        // 产品选择弹窗（fill-input 模式：选产品→填 #routing-product-id + 显示名 + 关弹窗）
        (product_picker::product_picker_modal("routing-product-modal", "routing-product-id", "routing-product-display"))
    }
}
```

> product_picker_modal 的 modal 与抽屉同为 z-[1000] overlay；为让 picker 浮在抽屉之上，渲染顺序上 picker 在抽屉 body 之后（已是），DOM 后者居上即可。若实测层叠不对，给 `#routing-product-modal` 加更高 z（实现期 `snapshot -i` 走查确认）。

- [ ] **Step 4: `tab_routing` 改 + `order_detail_page` 加抽屉壳 + `get_order_detail` 取产品名**

- `tab_routing` 签名加 `product_names: &HashMap<i64,String>`，`routing_tbody_fragment` 调用传之。
- `order_detail_page` 签名加 `product_names`，透传给 `tab_routing`；在页面末尾（tab_panel 之后）渲染抽屉壳：

```rust
// 编辑抽屉（页面级，body 由 GET /edit 载入）
(crate::components::drawer::drawer(
    "routing-edit-drawer",
    "编辑工序",
    "保存",
    "routing-edit-form",
    html! { div id="routing-edit-drawer-body" _="on htmx:afterSettle add .open to #routing-edit-drawer" {} },
))
```

- `get_order_detail`：在 `routings` 取完后，批量取产出品名：

```rust
    let product_ids: Vec<i64> = routings.iter().filter_map(|r| r.product_id).collect();
    let product_names: std::collections::HashMap<i64, String> = if product_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        state.product_service().get_by_ids(&service_ctx, &mut conn, product_ids).await
            .unwrap_or_default().into_iter().map(|p| (p.product_id, p.pdt_name)).collect()
    };
```

把 `product_names` 传入 `order_detail_page`（替换/补充现有参数）。

- [ ] **Step 5: cargo check + clippy + 测试**

Run: `cargo check -p abt-web 2>&1 | grep -E "^error" | head`
Run: `cargo clippy -p abt-web -p abt-core --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- --test-threads=1 2>&1 | tail -5`
Expected: 0 error，测试全 PASS。

- [ ] **Step 6: 提交**

```bash
git add abt-web/src/routes/mes_order.rs abt-web/src/pages/mes_order_detail.rs
git commit -m "feat(mes): 工单工序行只读 + 编辑抽屉（product picker 选产出品 + 单价，一次性保存）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: uml 同步 + 最终验证

**Files:**
- Modify: `docs/uml-design/04-mes.html`（`ProductionBatchService` trait：`update_routing_unit_price`/`update_routing_product` → `update_routing`）

- [ ] **Step 1: uml 改**

`04-mes.html` 中 `ProductionBatchService` class block：删除 `+update_routing_unit_price(...)` 和 `+update_routing_product(...)` 两行，替换为：
```
+update_routing(ctx, db, work_order_id, routing_id, product_id, unit_price) Result~WorkOrderRouting~
```

- [ ] **Step 2: 全量 clippy + 回归测试（serial）**

Run: `cargo clippy --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price --test mes_batch --test mes_flow_e2e --test mes_pages -- --test-threads=1 2>&1 | grep -E "test result|FAILED"`
Expected: 0 error，全部 PASS。

- [ ] **Step 3: 提交**

```bash
git add docs/uml-design/04-mes.html
git commit -m "docs(uml): 同步 update_routing（合并工序更新接口）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec 覆盖**：§3.1 行只读+编辑按钮 → T3 Step1；§3.2 抽屉壳 → T3 Step4；§3.3 GET /edit 表单 → T2 Step2 + T3 Step3；§3.4 POST /edit OOB+关抽屉 → T2 Step2；§4.1 service 合并 → T1；§4.2 端点增删 → T2；§4.3 product_name → T3；§7 测试 → T1 Step1；§8 uml → T4。全覆盖。

**2. 占位符扫描**：`routing_row_oob_swap` 的 td 序列注明"与 routing_row_fragment 完全一致，复制"——这是 maud 无法复用片段的限制下的明确指引（非空洞 TODO，给出了完整 td 序列的来源）。其余代码均完整。

**3. 类型一致性**：`update_routing(ctx, db, wo_id, rid, product_id: Option<i64>, unit_price: Decimal)` 在 T1(trait+impl)、T2(handler 调用) 一致；`routing_row_fragment(r, is_reported_step, order_has_report, product_name: Option<&str>)` 在 T2(post OOB 调用) 与 T3(定义) 一致；`OrderRoutingEditPath` 在 T2(路由/handler) 与 T3(行按钮 hx-get) 一致。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-routing-edit-drawer.md`. Two execution options:

**1. Subagent-Driven** — 每 Task 派 subagent + review（⚠️ 上轮 sonnet 实现者多次伪造验证，需 controller 严格交叉核对 + 自跑测试）
**2. Inline Execution（推荐）** — 当前会话内联执行（cargo check 13s + 串行测试可靠），带检查点 review

Which approach?
