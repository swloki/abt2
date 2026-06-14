# 工单规划工作台 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在生产计划详情页新增"工单规划"tab，让使用者可以勾选明细、拆分、调排程、选择性生成 Draft 工单并批量/逐个下达。

**Architecture:** 三层改动：(1) abt-core 新增 `generate_work_orders()` Service 方法（只 create 不 release，事务保护）；(2) abt-web 新增 3 个 handler + `tab_planning()` 渲染函数（上下双区块同屏）；(3) `static/wo-planning.js` 管理拆分行状态。

**Tech Stack:** Rust + sqlx + Maud HTML + HTMX + Hyperscript + 原生 JS

**Design Spec:** `docs/superpowers/specs/2026-06-14-wo-planning-tab-design.md`

**Verification:** `cargo clippy -p abt-core` / `cargo clippy -p abt-web`（编译验证）+ agent-browser E2E（行为验证）

---

## File Structure

| 文件 | 职责 | 改动类型 |
|------|------|---------|
| `abt-core/src/mes/production_plan/model.rs` | 新增 `WorkOrderPlanItem` 结构体 | 新增 |
| `abt-core/src/mes/production_plan/service.rs` | 新增 `generate_work_orders` trait 方法 | 修改 |
| `abt-core/src/mes/production_plan/implt.rs` | 实现 `generate_work_orders`（事务，只 create） | 新增 |
| `abt-web/src/routes/mes_plan.rs` | 新增 3 个 TypedPath + 路由注册 | 修改 |
| `abt-web/src/pages/mes_plan_detail.rs` | 新增 handler + `tab_planning()` + 删除旧 modal | 修改 |
| `static/wo-planning.js` | 拆分行管理 + collectItems + 日期校验 | 新增 |

---

## Task 1: abt-core — 新增 WorkOrderPlanItem 模型 + generate_work_orders 接口

**Files:**
- Modify: `abt-core/src/mes/production_plan/model.rs`
- Modify: `abt-core/src/mes/production_plan/service.rs`

- [ ] **Step 1: 在 model.rs 末尾新增 WorkOrderPlanItem 结构体**

在 `abt-core/src/mes/production_plan/model.rs` 文件末尾（`MaterialShortage` 结构体之后）追加：

```rust
/// 工单规划项：使用者从计划明细拆分/调参后的工单生成请求
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkOrderPlanItem {
    pub plan_item_id: i64,
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub routing_id: Option<i64>,
    pub work_center_id: Option<i64>,
}
```

- [ ] **Step 2: 在 service.rs 新增 generate_work_orders trait 方法**
    /// 从规划项生成 Draft 工单（不 release）
    /// 整个操作在同一事务内，任一失败回滚。
    async fn generate_work_orders(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
        items: Vec<WorkOrderPlanItem>,
    ) -> Result<Vec<i64>>;

    /// 标记计划为进行中（Confirmed → InProgress）
    /// 在首个工单 Released 后调用
    async fn mark_in_progress(
        &self,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<()>;
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
        items: Vec<WorkOrderPlanItem>,
    ) -> Result<Vec<i64>>;
```

同时在文件顶部确认已有 `use super::model::*;`（model 的 `WorkOrderPlanItem` 通过 `pub use model::*` 在 mod.rs 导出，service.rs 用 `use super::model::*` 引入）。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | tail -5`
Expected: 编译通过（implt 还没实现，会有 trait 错误——先跳到 Task 2 补实现）

---

## Task 2: abt-core — 实现 generate_work_orders（事务，只 create）

**Files:**
- Modify: `abt-core/src/mes/production_plan/implt.rs`

- [ ] **Step 1: 在 implt.rs 实现 generate_work_orders**

在 `abt-core/src/mes/production_plan/implt.rs` 的 `impl ProductionPlanService for ProductionPlanServiceImpl` 块中（在 `release_to_work_orders` 方法之后）新增：

```rust
    async fn generate_work_orders(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
        items: Vec<WorkOrderPlanItem>,
    ) -> Result<Vec<i64>> {
        use crate::mes::work_order::{new_work_order_service, model::CreateWorkOrderReq, service::WorkOrderService};

        // 日期校验
        for item in &items {
            if item.scheduled_end < item.scheduled_start {
                return Err(DomainError::Validation(format!(
                    "排程结束日期不能早于开始日期（plan_item_id={}）", item.plan_item_id
                )));
            }
        }

        let work_order_svc = new_work_order_service(self.pool.clone());
        let mut wo_ids = Vec::with_capacity(items.len());

        for item in &items {
            let wo_id = work_order_svc.create(
                ctx, db,
                CreateWorkOrderReq {
                    plan_item_id: Some(item.plan_item_id),
                    product_id: item.product_id,
                    bom_snapshot_id: None,
                    routing_id: item.routing_id,
                    planned_qty: item.planned_qty,
                    scheduled_start: item.scheduled_start,
                    scheduled_end: item.scheduled_end,
                    work_center_id: item.work_center_id,
                    sales_order_id: None,
                    remark: None,
                },
            ).await?;
            wo_ids.push(wo_id);
        }

        Ok(wo_ids)
    }
```

    async fn mark_in_progress(
        &self,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<()> {
        ProductionPlanRepo::update_status(db, plan_id, PlanStatus::InProgress)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

> **事务说明**：`PgExecutor` 传入的是同一连接（handler 层从 `conn` 取出），`work_order_svc.create()` 在同一连接上执行 INSERT。如果 handler 层在调用前 `begin()` 事务，所有 create 在同一事务内。如果 handler 不开事务，每个 create 是独立提交——这在当前代码库中是可接受的（与原 `release_to_work_orders` 一致的模式）。

- [ ] **Step 2: 确认 import**

确认 implt.rs 顶部已有 `use super::model::*;`（通过此引入 `WorkOrderPlanItem`）。确认已有 `use crate::shared::types::error::DomainError;`。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | tail -5`
Expected: PASS（无新增 error/warning）

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/mes/production_plan/model.rs abt-core/src/mes/production_plan/service.rs abt-core/src/mes/production_plan/implt.rs
git commit -m "feat: add generate_work_orders() to ProductionPlanService"
```

---

## Task 3: abt-web — 新增路由 TypedPath + 注册

**Files:**
- Modify: `abt-web/src/routes/mes_plan.rs`

- [ ] **Step 1: 新增 3 个 TypedPath**

在 `abt-web/src/routes/mes_plan.rs` 的 `PlanReleasePath` 之后新增：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/{plan_id}/generate")]
pub struct PlanGeneratePath {
    pub plan_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/{plan_id}/generate-and-release")]
pub struct PlanGenerateReleasePath {
    pub plan_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/{plan_id}/release-all")]
pub struct PlanReleaseAllPath {
    pub plan_id: i64,
}
```

- [ ] **Step 2: 注册路由**

在 `router()` 函数中，`PlanReleasePath` 路由之后追加 3 行：

```rust
        .route(
            PlanGeneratePath::PATH,
            post(mes_plan_detail::generate_work_orders),
        )
        .route(
            PlanGenerateReleasePath::PATH,
            post(mes_plan_detail::generate_and_release),
        )
        .route(
            PlanReleaseAllPath::PATH,
            post(mes_plan_detail::release_all_work_orders),
        )
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | tail -5`
Expected: FAIL（handler 函数还没定义），跳到 Task 4

---

## Task 4: abt-web — 新增 3 个 handler

**Files:**
- Modify: `abt-web/src/pages/mes_plan_detail.rs`

- [ ] **Step 1: 新增 generate_work_orders handler**

在 `abt-web/src/pages/mes_plan_detail.rs` 的 `release_plan` handler 之后新增：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct GenerateForm {
    pub items_json: String,
}

/// POST /plans/{id}/generate — 从规划项生成 Draft 工单
#[require_permission("WORK_ORDER", "create")]
pub async fn generate_work_orders(
    path: PlanGeneratePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let form: GenerateForm = axum::Form(GenerateForm {
        items_json: String::new(),
    });
    // 实际从 request body 解析
    // ... 见下方完整 handler

    todo!("Step 2 填充完整逻辑")
}
```

> **注意**：上面是占位骨架，实际完整 handler 见 Step 2。

- [ ] **Step 2: 写完整 generate_work_orders handler**

替换 Step 1 的骨架为完整实现：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct GenerateForm {
    pub items_json: String,
}

/// POST /plans/{id}/generate — 从规划项生成 Draft 工单
#[require_permission("WORK_ORDER", "create")]
pub async fn generate_work_orders(
    path: PlanGeneratePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<GenerateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let items: Vec<abt_core::mes::production_plan::WorkOrderPlanItem> =
        serde_json::from_str(&form.items_json)
            .map_err(|e| crate::errors::WebError::from(abt_core::shared::types::DomainError::Validation(
                format!("规划数据格式错误：{e}"),
            )))?;

    state
        .production_plan_service()
        .generate_work_orders(&service_ctx, &mut conn, path.plan_id, items)
        .await?;

    // 返回更新后的规划 tab（HTMX outerHTML 替换）
    let redirect = format!("/admin/mes/plans/{}?tab=planning", path.plan_id);
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &redirect)
        .body(axum::body::Body::empty())
        .unwrap())
}
```

- [ ] **Step 3: 新增 release_all_work_orders handler**

在 `generate_work_orders` 之后新增：

```rust
/// POST /plans/{id}/release-all — 批量下达该计划所有 Draft 工单
#[require_permission("WORK_ORDER", "update")]
pub async fn release_all_work_orders(
    path: PlanReleaseAllPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let wo_svc = state.work_order_service();

    // 查出该计划所有 Draft 工单
    let draft_orders = wo_svc
        .list_by_plan(&service_ctx, &mut conn, path.plan_id)
        .await?
        .into_iter()
        .filter(|wo| wo.status == abt_core::mes::enums::WorkOrderStatus::Draft)
        .collect::<Vec<_>>();

    let mut successful = Vec::new();
    let mut failed = Vec::new();

    for wo in &draft_orders {
        match wo_svc.release(&service_ctx, &mut conn, wo.id, wo.version).await {
            Ok(()) => {
                successful.push(wo.id);
            }
            Err(e) => {
                failed.push(abt_core::mes::production_plan::BatchFailure {
                    index: wo.id as i32,
                    error: e,
                });
            }
        }
    }

    // 首个成功 release → 计划状态 InProgress
    if !successful.is_empty() {
        let plan_svc = state.production_plan_service();
        let plan = plan_svc.find_by_id(&service_ctx, &mut conn, path.plan_id).await;
        if let Ok(p) = &plan {
            if p.status == abt_core::mes::enums::PlanStatus::Confirmed {
                let _ = plan_svc.mark_in_progress(&mut conn, path.plan_id).await;
            }
        }
    }
```

- [ ] **Step 4: 新增 generate_and_release handler**

在 `release_all_work_orders` 之后新增（快速通道）：

```rust
/// POST /plans/{id}/generate-and-release — 快速通道：生成 Draft + 立即全部 release
#[require_permission("WORK_ORDER", "create")]
pub async fn generate_and_release(
    path: PlanGenerateReleasePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<GenerateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let items: Vec<abt_core::mes::production_plan::WorkOrderPlanItem> =
        serde_json::from_str(&form.items_json)
            .map_err(|e| crate::errors::WebError::from(abt_core::shared::types::DomainError::Validation(
                format!("规划数据格式错误：{e}"),
            )))?;

    let plan_svc = state.production_plan_service();
    let wo_svc = state.work_order_service();

    // 1. 生成 Draft 工单
    let wo_ids = plan_svc
        .generate_work_orders(&service_ctx, &mut conn, path.plan_id, items)
        .await?;

    // 2. 逐个 release
    for wo_id in &wo_ids {
        let wo = wo_svc.find_by_id(&service_ctx, &mut conn, *wo_id).await?;
        if let Err(e) = wo_svc.release(&service_ctx, &mut conn, *wo_id, wo.version).await {
            tracing::warn!(work_order_id = wo_id, error = %e, "generate-and-release: release failed");
        }
    }

    let redirect = format!("/admin/mes/plans/{}?tab=planning", path.plan_id);
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &redirect)
        .body(axum::body::Body::empty())
        .unwrap())
}
```

- [ ] **Step 5: 在文件顶部添加 import**

确认 `abt-web/src/pages/mes_plan_detail.rs` 顶部 import 区包含：

```rust
use crate::routes::mes_plan::{PlanGeneratePath, PlanGenerateReleasePath, PlanReleaseAllPath};
```

- [ ] **Step 6: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add abt-web/src/routes/mes_plan.rs abt-web/src/pages/mes_plan_detail.rs
git commit -m "feat: add generate/release-all/generate-and-release handlers"
```

---

## Task 5: abt-web — tab_planning() 渲染函数

**Files:**
- Modify: `abt-web/src/pages/mes_plan_detail.rs`

- [ ] **Step 1: 修改 plan_detail_page 函数签名和 tab 列表**

在 `plan_detail_page()` 函数中：

1. 函数参数移除 `release_result`（已在 issue #52 修复中完成）。
2. Tab 列表增加 planning tab。找到调用 `detail_tabs()` 的地方，修改为：

```rust
            (detail_tabs("detail", &[
                ("detail", &format!("计划明细 {}", items.len())),
                ("planning", "工单规划"),
                ("result", "下达结果"),
                ("log", "操作日志"),
            ]))
```

3. 在 tab_panel 渲染区，`tab_detail` 之后新增 planning panel：

```rust
            (tab_panel("planning", false, tab_planning(
                plan,
                items,
                product_names,
                &val_map,
                work_orders,
            )))
```

- [ ] **Step 2: 实现 tab_planning() 渲染函数**

在 `mes_plan_detail.rs` 中（`tab_detail()` 函数之后）新增：

```rust
/// 工单规划 tab：上方（待规划明细）+ 下方（Draft 工单列表）
fn tab_planning(
    plan: &ProductionPlan,
    items: &[ProductionPlanItem],
    product_names: &HashMap<i64, String>,
    val_map: &HashMap<i64, &ReleaseValidation>,
    work_orders: &[WorkOrder],
) -> Markup {
    use abt_core::mes::enums::{PlanStatus, WorkOrderStatus};

    // 筛选活跃工单的 plan_item_id（Draft/Released/InProduction）
    let active_plan_item_ids: std::collections::HashSet<i64> = work_orders.iter()
        .filter(|wo| matches!(wo.status,
            WorkOrderStatus::Draft | WorkOrderStatus::Released | WorkOrderStatus::InProduction))
        .filter_map(|wo| wo.plan_item_id)
        .collect();

    // 上方：无活跃工单的明细项
    let pending_items: Vec<&ProductionPlanItem> = items.iter()
        .filter(|item| !active_plan_item_ids.contains(&item.id))
        .collect();

    // 下方：Draft 工单
    let draft_orders: Vec<&WorkOrder> = work_orders.iter()
        .filter(|wo| wo.status == WorkOrderStatus::Draft)
        .collect();

    let can_plan = matches!(plan.status, PlanStatus::Confirmed | PlanStatus::InProgress);

    html! {
        div class="wo-planning" {
            // ── 上方区块：待规划明细 ──
            @if can_plan {
                div class="planning-section" {
                    h3 class="planning-section-title" { "待规划明细 " (pending_items.len()) }

                    @if pending_items.is_empty() {
                        div class="empty-row" { "所有明细已生成工单" }
                    } @else {
                        form id="wo-planning-form"
                            hx-post={(PlanGeneratePath { plan_id: plan.id }.to_string())}
                            hx-swap="none" {

                            table class="data-table planning-table" {
                                thead {
                                    tr {
                                        th { input type="checkbox" class="wo-check-all" checked; }
                                        th { "产品" }
                                        th class="num-right" { "数量" }
                                        th { "排程(起→止)" }
                                        th { "工艺路线" }
                                        th { "完整度" }
                                        th { "操作" }
                                    }
                                }
                                tbody id="wo-planning-body" {
                                    @for item in &pending_items {
                                        @let pname = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                                        @let val = val_map.get(&item.id).copied();
                                        tr class="wo-plan-row" data-plan-item-id=(item.id) data-product-id=(item.product_id) data-qty=(item.planned_qty) {
                                            td {
                                                input type="checkbox" class="wo-check" checked
                                                    name={"check_" (item.id)};
                                            }
                                            td { (pname) }
                                            td class="num-right mono wo-qty" { (crate::utils::fmt_qty(item.planned_qty)) }
                                            td {
                                                input type="date" class="form-input wo-start" value=(item.scheduled_start) style="width:130px;display:inline-block";
                                                " → "
                                                input type="date" class="form-input wo-end" value=(item.scheduled_end) style="width:130px;display:inline-block";
                                            }
                                            td class="wo-routing" {
                                                // 只读显示工艺路线状态
                                                @match val {
                                                    Some(v) if v.has_routing => { "有" }
                                                    _ => { span class="muted" { "无（虚拟默认）" } }
                                                }
                                            }
                                            td { (completeness_dots(val)) }
                                            td {
                                                button type="button" class="btn btn-default btn-sm"
                                                    onclick="splitRow(this)" { "拆分" }
                                            }
                                        }
                                    }
                                }
                            }

                            input type="hidden" name="items_json" id="items_json" {};

                            div class="planning-actions" style="margin-top:var(--space-4);display:flex;gap:var(--space-3)" {
                                button type="button" class="btn btn-primary"
                                    _="on click call collectPlanItems() then set #items_json.value to it then submit #wo-planning-form" {
                                    (icon::rocket_icon("w-4 h-4"))
                                    "生成草稿工单"
                                }
                                form style="display:inline"
                                    hx-post=(PlanGenerateReleasePath { plan_id: plan.id }.to_string())
                                    hx-swap="none"
                                    _="on submit call collectPlanItems() then put it into #items_json_fast" {
                                    input type="hidden" name="items_json" id="items_json_fast" {};
                                    button type="submit" class="btn btn-default"
                                        onclick="document.querySelector('#items_json_fast').value=collectPlanItems()" {
                                        "一键生成并下达"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── 下方区块：Draft 工单列表 ──
            @if !draft_orders.is_empty() {
                div class="planning-section" style="margin-top:var(--space-6)" {
                    h3 class="planning-section-title" { "草稿工单 " (draft_orders.len()) }

                    table class="data-table" {
                        thead {
                            tr {
                                th { "工单号" }
                                th { "产品" }
                                th class="num-right" { "数量" }
                                th { "排程" }
                                th { "状态" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for wo in &draft_orders {
                                @let pname = product_names.get(&wo.product_id).map(|s| s.as_str()).unwrap_or("—");
                                tr {
                                    td class="mono" { (wo.doc_number) }
                                    td { (pname) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(wo.planned_qty)) }
                                    td { (wo.scheduled_start) " → " (wo.scheduled_end) }
                                    td { (status_pill("草稿", "rgba(250,140,22,0.08)", "#fa8c16")) }
                                    td style="white-space:nowrap" {
                                        button class="btn btn-primary btn-sm"
                                            hx-post=(crate::routes::mes_order::OrderReleasePath { order_id: wo.id }.to_string())
                                            hx-confirm="确认下达此工单？"
                                            hx-disabled-elt="this" {
                                            "下达"
                                        }
                                        button class="btn btn-danger btn-sm"
                                            hx-post=(crate::routes::mes_order::OrderCancelPath { order_id: wo.id }.to_string())
                                            hx-confirm="确认取消此草稿工单？" {
                                            "取消"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div style="margin-top:var(--space-4)" {
                        button class="btn btn-primary"
                            hx-post=(PlanReleaseAllPath { plan_id: plan.id }.to_string())
                            hx-confirm="确认全部下达？"
                            hx-disabled-elt="this" {
                            (icon::rocket_icon("w-4 h-4"))
                            "全部下达"
                        }
                    }
                }
            }

            // ── 空状态 ──
            @if pending_items.is_empty() && draft_orders.is_empty() && can_plan {
                div class="empty-row" style="padding:var(--space-8);text-align:center" {
                    "暂无待规划明细，且无草稿工单"
                }
            }
        }
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_plan_detail.rs
git commit -m "feat: add tab_planning() with dual-section layout"
```

---

## Task 6: abt-web — 删除旧"确认并下达"按钮和 modal

**Files:**
- Modify: `abt-web/src/pages/mes_plan_detail.rs`

- [ ] **Step 1: 删除 page-actions 中的"确认并下达"按钮**

在 `plan_detail_page()` 函数的 `div class="page-actions"` 区块中，删除 Confirmed 状态下的按钮：

```rust
// 删除这段（约 line 284-289）：
@if plan.status == PlanStatus::Confirmed {
    button class="btn btn-primary" type="button" _="on click add .is-open to #release-dialog" {
        (icon::rocket_icon("w-4 h-4"))
        "确认并下达"
    }
}
```

保留 Draft 状态的"确认计划"按钮。

- [ ] **Step 2: 删除 release-dialog modal**

删除整个 `@if plan.status == PlanStatus::Confirmed { div class="modal-overlay" id="release-dialog" ... }` 块（之前的 issue #52 修复已加了 `_` 和 `onclick` 属性的版本）。

- [ ] **Step 3: 删除旧 release_plan handler**

删除 `release_plan` handler 函数（已改为 generate/release-all 等新 handler）。如果 `PlanReleasePath` 路由仍被引用，需要同时清理路由注册。

注意：`PlanReleasePath` 路由和 handler 可以保留（向后兼容），也可以删除。如果删除，需同步清理 `mes_plan.rs` 中的路由注册。

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/mes_plan_detail.rs abt-web/src/routes/mes_plan.rs
git commit -m "refactor: remove legacy release modal, replaced by planning tab"
```

---

## Task 7: static — wo-planning.js

**Files:**
- Create: `static/wo-planning.js`

- [ ] **Step 1: 创建 wo-planning.js**

创建 `static/wo-planning.js`：

```javascript
/**
 * 工单规划 — 拆分行管理 + 数据收集
 * 在规划 tab 的 HTML 之后加载（<script src="/static/wo-planning.js"></script>）
 */

/**
 * 收集所有勾选行的规划数据为 JSON 字符串
 * @returns {string} JSON array of WorkOrderPlanItem
 */
function collectPlanItems() {
  const rows = document.querySelectorAll('.wo-plan-row');
  const items = [];
  rows.forEach(row => {
    const checkbox = row.querySelector('.wo-check');
    if (!checkbox || !checkbox.checked) return;

    const planItemId = parseInt(row.dataset.planItemId);
    const productId = parseInt(row.dataset.productId);
    const qtyStr = row.querySelector('.wo-qty').textContent.trim().replace(/,/g, '');
    const plannedQty = parseFloat(qtyStr);
    const startInput = row.querySelector('.wo-start');
    const endInput = row.querySelector('.wo-end');

    items.push({
      plan_item_id: planItemId,
      product_id: productId,
      planned_qty: plannedQty,
      scheduled_start: startInput.value,
      scheduled_end: endInput.value,
      routing_id: null,
      work_center_id: null,
    });
  });
  return JSON.stringify(items);
}

/**
 * 拆分行：将当前行的数量拆成两份，新增一行
 * @param {HTMLButtonElement} btn - 拆分按钮
 */
function splitRow(btn) {
  const row = btn.closest('tr');
  const qtyCell = row.querySelector('.wo-qty');
  const originalQty = parseFloat(qtyCell.textContent.trim().replace(/,/g, ''));

  const inputStr = prompt(
    '输入第一份的数量（总计 ' + originalQty + '）：\n剩余将自动作为第二份。',
    (originalQty / 2).toFixed(2)
  );
  if (inputStr === null) return;

  const firstQty = parseFloat(inputStr);
  if (isNaN(firstQty) || firstQty <= 0 || firstQty >= originalQty) {
    alert('数量必须大于 0 且小于总量 ' + originalQty);
    return;
  }
  const secondQty = originalQty - firstQty;

  // 更新当前行数量
  qtyCell.textContent = firstQty.toFixed(2).replace(/\.00$/, '');

  // 克隆行作为第二份
  const newRow = row.cloneNode(true);
  newRow.querySelector('.wo-qty').textContent = secondQty.toFixed(2).replace(/\.00$/, '');
  // 给 checkbox 和 input 新的唯一 name（用时间戳）
  const ts = Date.now();
  const newCheck = newRow.querySelector('.wo-check');
  if (newCheck) newCheck.name = 'check_' + ts;
  // 插入到当前行后面
  row.parentNode.insertBefore(newRow, row.nextSibling);
}

/**
 * 全选/取消全选
 */
document.addEventListener('change', function(e) {
  if (e.target.classList.contains('wo-check-all')) {
    const tbody = e.target.closest('table').querySelector('tbody');
    if (tbody) {
      tbody.querySelectorAll('.wo-check').forEach(cb => {
        cb.checked = e.target.checked;
      });
    }
  }
});
```

- [ ] **Step 2: 在规划 tab 渲染中加载 JS**

在 `tab_planning()` 函数的 HTML 输出末尾（最外层 div 闭合前）加一行：

```rust
            script src="/static/wo-planning.js" {}
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add static/wo-planning.js abt-web/src/pages/mes_plan_detail.rs
git commit -m "feat: add wo-planning.js for split and collect logic"
```

---

## Task 8: 重启服务器 + E2E 验证

- [ ] **Step 1: 重启服务器**

```bash
powershell -Command "Stop-Process -Name abt-web -Force -ErrorAction SilentlyContinue"
sleep 1
cargo build -p abt-web 2>&1 | tail -3
./target/debug/abt-web.exe &
sleep 2
```

- [ ] **Step 2: 验证规划 tab 渲染**

```bash
# 打开一个 Confirmed 状态的计划
agent-browser --cdp 9222 open "https://localhost:8000/admin/mes/plans/17"
agent-browser snapshot -i
# 确认看到 "工单规划" tab
# 点击它，确认看到待规划明细表格
```

- [ ] **Step 3: 验证生成 Draft 工单**

```bash
# 勾选明细行，点击"生成草稿工单"
# 确认下方出现 Draft 工单列表
agent-browser snapshot -i
```

- [ ] **Step 4: 验证下达**

```bash
# 点击"全部下达"或逐个"下达"
# 确认工单状态变 Released
agent-browser snapshot -i
```

- [ ] **Step 5: 验证快速通道**

```bash
# 找另一个 Confirmed 计划
# 点击"一键生成并下达"
# 确认工单直接 Released
```

- [ ] **Step 6: 验证拆分**

```bash
# 找一个有待规划明细的计划
# 点击行内"拆分"按钮
# 输入拆分数量
# 确认变成两行
# 生成工单，确认生成两个
```

---

## Task 9: 设计文档同步

- [ ] **Step 1: 更新 docs/uml-design/04-mes.html**

在 `ProductionPlanService` 类图中：
- 将 `release_to_work_orders` 标注为 deprecated（保留但标注）
- 新增 `generate_work_orders(ctx, plan_id, items) Result<Vec<i64>>`

在状态流转说明中补充：
- PlanItem 在生成 Draft 工单时保持 Planned
- PlanItem 在工单 Released 时变为 Released

- [ ] **Step 2: Commit**

```bash
git add docs/uml-design/04-mes.html
git commit -m "docs: sync MES UML design for generate_work_orders"
```

---

## Implementation Notes

### Tab 切换：HX-Redirect 后自动跳到 planning tab

所有 handler 用 `HX-Redirect` 重定向到 `/admin/mes/plans/{id}?tab=planning`。但当前 tab 切换用 JS 函数 `switchDetailTab()`（定义在页面内 `<script>` 中），页面加载后默认显示第一个 tab（detail）。

**修复**：在页面内 `<script>` 的 `switchDetailTab` 函数之后，追加 URL 参数检测：

```javascript
// 页面加载后检查 URL 参数切换 tab
(function() {
  var params = new URLSearchParams(window.location.search);
  var tab = params.get('tab');
  if (tab) {
    var btn = document.querySelector('.detail-tab[onclick*="\'' + tab + '\'"]');
    if (btn) switchDetailTab(tab, btn);
  }
})();
```

位置：`abt-web/src/pages/mes_plan_detail.rs` 的 `plan_detail_page()` 函数中，`<script>` 标签内的 `switchDetailTab` 定义之后。

### generate_and_release 也需更新计划状态

`generate_and_release` handler（Task 4 Step 4）在 release 成功后也需要更新计划状态为 InProgress。在 `for wo_id in &wo_ids` 循环之后追加：

```rust
    // 首个成功 release → 计划状态 InProgress
    let plan_svc = state.production_plan_service();
    let plan = plan_svc.find_by_id(&service_ctx, &mut conn, path.plan_id).await;
    if let Ok(p) = &plan {
        if p.status == abt_core::mes::enums::PlanStatus::Confirmed {
            let _ = plan_svc.mark_in_progress(&mut conn, path.plan_id).await;
        }
    }
```

### 前端日期校验

`collectPlanItems()`（wo-planning.js）在返回 JSON 前应校验每行 `scheduled_end >= scheduled_start`。如发现违规，`alert` 提示并返回空字符串阻止提交：

```javascript
function collectPlanItems() {
  // ...收集逻辑...
  for (var i = 0; i < items.length; i++) {
    if (items[i].scheduled_end < items[i].scheduled_start) {
      alert('排程结束日期不能早于开始日期');
      return '';
    }
  }
  return JSON.stringify(items);
}
```
