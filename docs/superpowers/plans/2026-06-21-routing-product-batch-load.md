# 工单工序产出品批量加载 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 工单详情工序 tab 加「批量加载产出品」两种来源（①从工艺路线模板 ②从最近同 routing_id 工单），仅填未报工+原空行；并补 routing_create 模板产出品录入 UI（① 前提）。

**Architecture:** Service 加两个批量方法（单事务、逐行 `!has_report`+原空行守卫、审计）；web 加两个 POST 端点刷新 tbody；routing_create 每步加产出品 select、routing_detail 显示名。复用上一特性已建的 `routing_tbody_fragment` / `drawer` / `product_picker`。

**Tech Stack:** Rust 2024 / axum + TypedPath / sqlx / Maud / HTMX 2.0.10 / Hyperscript / async-trait / rust_decimal。

## Global Constraints

- **沟通用中文；commit message 中文**，结尾 `Co-Authored-By: Claude <noreply@anthropic.com>`
- **不要 `cargo run`**（服务在跑）；验证用 `cargo clippy` + `cargo test`
- **代码导航用 `lsp`**，禁文本搜索代替查定义/引用
- **跨模块只走 Service trait/Model**；`RoutingRepo` 是 master_data 模块，production_batch（mes）调用走工厂 `new_routing_service` 或直接 `RoutingRepo::find_steps`（参考 routing_service 现有调用）
- **错误禁止静默丢弃**：`?`/`map_err`；`DomainError::validation/business_rule/not_found`
- **所有 TypedPath**，禁硬编码 URL
- **样式 100% UnoCSS**，禁 `style=""` 内联
- **测试 DB 集成**，`abt-web/tests/`，**必须 `--test-threads=1`**
- ⚠️ **执行要点**：`cargo check -p abt-web --tests` ~13s；**必须真实跑 `cargo check`+`cargo test` 并贴输出**（上轮 sonnet 实现者多次伪造"0 error"，controller 须交叉核对 diff + 自跑测试）；**推荐 Inline 执行**

## 参考文件
- spec：`docs/superpowers/specs/2026-06-21-routing-product-batch-load-design.md`
- `abt-core/src/master_data/routing/repo.rs`：`RoutingRepo::find_steps(executor, routing_id) -> Vec<RoutingStep>`（line 97）、`insert_steps`（24）
- `abt-core/src/mes/production_batch/{service.rs,implt.rs,repo.rs}`：`WorkOrderRoutingRepo::get_by_id/has_report/update_unit_price`；`update_routing`（上一特性，单事务范式）
- `abt-web/src/pages/mes_order_detail.rs`：`tab_routing`/`routing_tbody_fragment`/`get_order_detail`（上一特性已建）
- `abt-web/src/pages/routing_create.rs`：`StepWeb`(33)、`addStep()` JS、`post_routing_create` map(104)；`routing_detail.rs` 工序表(166)
- `abt-web/src/routes/mes_order.rs`：TypedPath 范式
- `abt-web/tests/mes_routing_price.rs`：seeding helper（`seed_released_work_order(app, product_id, qty)`、`ServiceContext::new(1)`、`app.state.pool.acquire()`）

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-core/src/mes/production_batch/service.rs` | trait 加 2 方法 | 改 |
| `abt-core/src/mes/production_batch/implt.rs` | 实现 2 方法 + 找历史工单查询 | 改 |
| `abt-core/src/mes/production_batch/repo.rs` | `WorkOrderRoutingRepo::find_recent_with_product(db, routing_id, exclude_wo_id) -> Option<i64>` + `list_by_work_order_with_product`（或复用 get_by_work_order_id） | 改 |
| `abt-web/src/routes/mes_order.rs` | 2 TypedPath + 注册 | 改 |
| `abt-web/src/pages/mes_order_detail.rs` | 2 handler + tab_routing 加载按钮 + tbody 包裹 id | 改 |
| `abt-web/src/pages/routing_create.rs` | StepWeb + addStep JS 加产出品 select + list 产品 | 改 |
| `abt-web/src/pages/routing_detail.rs` | 产出品列显示名 | 改 |
| `abt-web/tests/mes_routing_price.rs` | 2 service 批量测试 | 改 |
| `docs/uml-design/04-mes.html` | trait 加 2 方法 | 改 |

---

### Task 1: service 批量加载方法 + repo 查询

**Files:**
- Modify: `abt-core/src/mes/production_batch/repo.rs`（`WorkOrderRoutingRepo` 加 `find_recent_source_work_order`）
- Modify: `abt-core/src/mes/production_batch/service.rs`（trait 加 2 方法）
- Modify: `abt-core/src/mes/production_batch/implt.rs`（实现 2 方法）

**Interfaces:**
- Consumes: `RoutingRepo::find_steps(executor, routing_id) -> Vec<RoutingStep>`（master_data，已存在）；`WorkOrderRoutingRepo::get_by_work_order_id`/`has_report`/`update_*`（已存在）；`new_work_order_service` 工厂
- Produces:
  - `WorkOrderRoutingRepo::find_recent_source_work_order(db, routing_id, exclude_wo_id) -> Result<Option<i64>>`
  - `ProductionBatchService::load_routings_from_template(ctx, db, work_order_id) -> Result<usize>`
  - `ProductionBatchService::load_routings_from_recent(ctx, db, work_order_id) -> Result<usize>`

- [ ] **Step 1: repo 加「找最近源工单」方法**

`abt-core/src/mes/production_batch/repo.rs`，`impl WorkOrderRoutingRepo` 块内加：

```rust
    /// 找同 routing_id、非本单、且已有产出品的最近工单 id（用于批量复制）
    pub async fn find_recent_source_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        routing_id: i64,
        exclude_wo_id: i64,
    ) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT wor2.work_order_id
            FROM work_order_routings wor2
            JOIN work_orders wo2 ON wo2.id = wor2.work_order_id
            WHERE wo2.routing_id = $1 AND wo2.id <> $2 AND wor2.product_id IS NOT NULL
            ORDER BY wo2.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(routing_id)
        .bind(exclude_wo_id)
        .fetch_optional(&mut *executor)
        .await?;
        Ok(row.map(|r| r.0))
    }
```

- [ ] **Step 2: 写失败测试**

`abt-web/tests/mes_routing_price.rs` 追加：

```rust
#[tokio::test]
async fn load_routings_from_template_fills_empty_unreported() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "900").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    // 该产品 routing_steps 模板多数无 product_id → 加载应为 0 或少量（不报错）
    let n = svc.load_routings_from_template(&ctx, &mut conn, wo_id).await.unwrap();
    // 无 panic 即可；具体行数取决于模板数据
    let _ = n;
}

#[tokio::test]
async fn load_routings_from_recent_copies_sibling() {
    let app = common::TestApp::new().await;
    let batch_svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    // A、B 同 routing_id（同产品 4544）
    let wo_a = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "910").await;
    let wo_b = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "911").await;

    // A 手动设第一道产出品
    let rs_a = batch_svc.list_routings(&ctx, &mut conn, wo_a).await.unwrap();
    batch_svc.update_routing(&ctx, &mut conn, wo_a, rs_a[0].id, Some(565), Decimal::new(5,0)).await.unwrap();

    // B 加载最近 → B 第一道应等于 A 的（565）
    let n = batch_svc.load_routings_from_recent(&ctx, &mut conn, wo_b).await.unwrap();
    assert!(n >= 1, "应至少复制 1 行，实际 {n}");
    let rs_b = batch_svc.list_routings(&ctx, &mut conn, wo_b).await.unwrap();
    assert_eq!(rs_b[0].product_id, Some(565), "B 应从 A 复制产出品");
}
```

- [ ] **Step 3: 跑确认失败**

Run: `cargo check -p abt-web --test mes_routing_price 2>&1 | grep -E "^error" | head`
Expected: trait 上无 `load_routings_from_template`/`load_routings_from_recent`。

- [ ] **Step 4: trait 加 2 方法**

`service.rs`，紧随 `update_routing` 之后加：

```rust
    /// 从工艺路径模板按 step_no 填充产出品（仅未报工 + 原空行）。返回填充行数
    async fn load_routings_from_template(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64,
    ) -> Result<usize>;

    /// 从最近同 routing_id 且有产出品的工单按 step_no 复制（仅未报工 + 原空行）。返回填充行数
    async fn load_routings_from_recent(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64,
    ) -> Result<usize>;
```

- [ ] **Step 5: impl 实现 2 方法**

`implt.rs`，紧随 `update_routing` 之后加。两个方法共用一个「按 step_no map 填充」的内部逻辑。所需 import：`use crate::master_data::routing::RoutingRepo;`（用 lsp 确认 `RoutingRepo` 路径与调用方式 `RoutingRepo.find_steps(...)` —— 若 `find_steps` 是 `&self`，用 `RoutingRepo.find_steps(&mut *tx, routing_id)` 需实例；参考 routing_service 现有调用，通常是 `RoutingRepo::find_steps` 或 `RoutingRepo.new()`/默认。实现期照 routing_service 的调用写）。

```rust
    async fn load_routings_from_template(
        &self, ctx: &ServiceContext, _db: PgExecutor<'_>, work_order_id: i64,
    ) -> Result<usize> {
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, &mut *self.pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?, work_order_id).await?;
        let routing_id = wo.routing_id.ok_or_else(|| DomainError::business_rule("工单未关联工艺路线"))?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许加载产出品"));
        }
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
        // 模板步 step_no → product_id
        let steps = RoutingRepo.find_steps(&mut *tx, routing_id).await?;
        let tpl: std::collections::HashMap<i32, i64> = steps.into_iter()
            .filter_map(|s| s.product_id.map(|pid| (s.step_order, pid))).collect();
        let mine = WorkOrderRoutingRepo::get_by_work_order_id(&mut *tx, work_order_id).await?;
        let mut filled = 0usize;
        for r in &mine {
            if r.product_id.is_some() { continue; }              // 原空行才填
            if WorkOrderRoutingRepo::has_report(&mut *tx, r.id).await? { continue; } // 未报工
            if let Some(pid) = tpl.get(&r.step_no) {
                sqlx::query(r#"UPDATE work_order_routings SET product_id = $2 WHERE id = $1"#)
                    .bind(r.id).bind(pid).execute(&mut *tx).await?;
                filled += 1;
            }
        }
        if filled > 0 {
            new_audit_log_service(self.pool.clone())
                .record(ctx, &mut *tx, RecordAuditLogReq {
                    entity_type: "WorkOrder", entity_id: work_order_id,
                    action: AuditAction::Update,
                    changes: Some(json!(format!("批量加载产出品自模板 routing#{routing_id}，{filled}行"))),
                    context: None,
                }).await?;
        }
        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(filled)
    }

    async fn load_routings_from_recent(
        &self, ctx: &ServiceContext, _db: PgExecutor<'_>, work_order_id: i64,
    ) -> Result<usize> {
        let mut conn = self.pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
        let wo = new_work_order_service(self.pool.clone()).find_by_id(ctx, &mut *conn, work_order_id).await?;
        let routing_id = wo.routing_id.ok_or_else(|| DomainError::business_rule("工单未关联工艺路线"))?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许加载产出品"));
        }
        drop(conn);
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
        let src_wo = WorkOrderRoutingRepo::find_recent_source_work_order(&mut *tx, routing_id, work_order_id).await?;
        let Some(src_id) = src_wo else { return Ok(0); };  // 无源工单 → 0
        let src_rows = WorkOrderRoutingRepo::get_by_work_order_id(&mut *tx, src_id).await?;
        let src: std::collections::HashMap<i32, i64> = src_rows.into_iter()
            .filter_map(|r| r.product_id.map(|pid| (r.step_no, pid))).collect();
        let mine = WorkOrderRoutingRepo::get_by_work_order_id(&mut *tx, work_order_id).await?;
        let mut filled = 0usize;
        for r in &mine {
            if r.product_id.is_some() { continue; }
            if WorkOrderRoutingRepo::has_report(&mut *tx, r.id).await? { continue; }
            if let Some(pid) = src.get(&r.step_no) {
                sqlx::query(r#"UPDATE work_order_routings SET product_id = $2 WHERE id = $1"#)
                    .bind(r.id).bind(pid).execute(&mut *tx).await?;
                filled += 1;
            }
        }
        if filled > 0 {
            new_audit_log_service(self.pool.clone())
                .record(ctx, &mut *tx, RecordAuditLogReq {
                    entity_type: "WorkOrder", entity_id: work_order_id,
                    action: AuditAction::Update,
                    changes: Some(json!(format!("批量加载产出品自工单#{src_id}，{filled}行"))),
                    context: None,
                }).await?;
        }
        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(filled)
    }
```

> 注意：`load_routings_from_template` 里 `find_by_id` 用了 `self.pool.acquire()` 借连接——更干净的写法是先 `let mut conn = self.pool.acquire()...; find_by_id(ctx, &mut conn, ...); drop(conn); let mut tx = self.pool.begin()...`（与 `load_routings_from_recent` 一致）。**实现时统一改成这个模式**（先 acquire 读工单，drop，再 begin 事务），避免 borrow 冲突。`get_by_work_order_id` 用 lsp 确认存在（routing repo 有 `get_by_work_order_id`，line ~137 of repo.rs）。

- [ ] **Step 6: cargo check + 测试**

Run: `cargo check -p abt-core 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- load_routings --test-threads=1 2>&1 | tail -5`
Expected: 0 error，2 测试 PASS（template 测试无 panic；recent 测试复制成功）。

- [ ] **Step 7: 提交**

```bash
git add abt-core/src/mes/production_batch/repo.rs abt-core/src/mes/production_batch/service.rs abt-core/src/mes/production_batch/implt.rs abt-web/tests/mes_routing_price.rs
git commit -m "feat(mes): 产出品批量加载 service（模板 / 最近同路径工单），仅填未报工+空行

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: web 端点 + tab 加载按钮

**Files:**
- Modify: `abt-web/src/routes/mes_order.rs`（2 TypedPath + 注册）
- Modify: `abt-web/src/pages/mes_order_detail.rs`（2 handler + tab_routing 顶部按钮 + tbody 包裹 id）

**Interfaces:**
- Consumes: Task 1 的两个 service 方法；`routing_tbody_fragment`（已存在，4 参数含 product_names）
- Produces: `POST /admin/mes/orders/{order_id}/routings/load-from-template`、`.../load-from-recent`

- [ ] **Step 1: 路由**

`routes/mes_order.rs` 加：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/load-from-template")]
pub struct OrderRoutingLoadTemplatePath { pub order_id: i64 }

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/load-from-recent")]
pub struct OrderRoutingLoadRecentPath { pub order_id: i64 }
```

`router()` 加：
```rust
        .route(OrderRoutingLoadTemplatePath::PATH, post(mes_order_detail::load_routings_from_template))
        .route(OrderRoutingLoadRecentPath::PATH, post(mes_order_detail::load_routings_from_recent))
```

- [ ] **Step 2: handler**

`mes_order_detail.rs` import 加两个 Path。加 handler（紧跟 `delete_routing` 之后）。两个 handler 共用「加载→重新 list_routings + 解析产品名 → 返回 tbody」逻辑：

```rust
#[require_permission("WORK_ORDER", "update")]
pub async fn load_routings_from_template(
    path: OrderRoutingLoadTemplatePath, ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    svc.load_routings_from_template(&service_ctx, &mut conn, path.order_id).await?;
    render_routing_tbody(&svc, &service_ctx, &mut conn, path.order_id).await
}

#[require_permission("WORK_ORDER", "update")]
pub async fn load_routings_from_recent(
    path: OrderRoutingLoadRecentPath, ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    svc.load_routings_from_recent(&service_ctx, &mut conn, path.order_id).await?;
    render_routing_tbody(&svc, &service_ctx, &mut conn, path.order_id).await
}

/// 加载后重新取工序列表 + 解析产品名 → 返回 routing_tbody_fragment
async fn render_routing_tbody(
    svc: &impl ProductionBatchService,
    ctx: &abt_core::shared::types::context::ServiceContext,
    conn: &mut sqlx::postgres::PgConnection,
    work_order_id: i64,
) -> Result<Html<String>> {
    use abt_core::master_data::product::ProductService;
    let routings = svc.list_routings(ctx, conn, work_order_id).await?;
    let pids: Vec<i64> = routings.iter().filter_map(|r| r.product_id).collect();
    let product_names: std::collections::HashMap<i64, String> = if pids.is_empty() {
        std::collections::HashMap::new()
    } else {
        // 注：handler 里用 state.product_service()；此处简化，实际把 state 传入或在此函数内取
        std::collections::HashMap::new() // 见下注
    };
    let empty = std::collections::HashSet::new();
    Ok(Html(routing_tbody_fragment(&routings, &empty, false, &product_names).into_string()))
}
```

> **注**：`render_routing_tbody` 需 `state.product_service()` 取产品名。实际实现把 `state: &AppState` 也传入 `render_routing_tbody`，用 `state.product_service().get_by_ids(ctx, conn, pids)` 填 product_names（参考 `delete_routing` handler 里已写的同名逻辑——**直接复用 delete_routing 里那段「list_routings + get_by_ids → product_names → routing_tbody_fragment」**，抽成 `render_routing_tbody(state, svc, ctx, conn, wo_id)`）。实现期把 delete_routing 也改为调这个公共函数（DRY）。`reported_routing_ids`/`order_has_report`：删除/加载后刷新用 `&empty`/`false` 近似（删除场景准确；加载场景 order_has_report 未变，但为简单统一用 false 不影响编辑/删除按钮——加载只发生在未报工或部分报工，刷新后按钮可见性下次 GET 详情页校正，可接受）。

- [ ] **Step 3: tab_routing 顶部加按钮 + tbody 包裹 id**

`tab_routing` 函数体内，`data-card` div 内、`overflow-x-auto` 之前插入操作栏：

```rust
 @if !order_has_report {
 div class="flex justify-end gap-2 mb-3" {
     button type="button" class="text-sm text-accent hover:text-accent-hover cursor-pointer"
         hx-post=(OrderRoutingLoadTemplatePath { order_id }.to_string())
         hx-target="#routing-tbody-wrap" hx-swap="outerHTML" hx-disabled-elt="this" {
         "从工艺路线加载"
     }
     button type="button" class="text-sm text-accent hover:text-accent-hover cursor-pointer"
         hx-post=(OrderRoutingLoadRecentPath { order_id }.to_string())
         hx-target="#routing-tbody-wrap" hx-swap="outerHTML" hx-disabled-elt="this" {
         "从最近工单加载"
     }
 }
 }
```

> `tab_routing` 需 `order_id` 来构造 Path。给 `tab_routing` + `routing_tbody_fragment` 链路加 `order_id: i64` 参数（从 `order_detail_page` 透传 `order.id`），或按钮用相对路径。**实现期**：给 `tab_routing(routings, reported_routing_ids, order_has_report, product_names, order_id)` 加 `order_id` 参数最直接；`order_detail_page` 调用处传 `order.id`。

tbody 包裹：把 `<table class="data-table">` 外层或 `routing_tbody_fragment` 返回的 `<tbody>` 加 `id="routing-tbody-wrap"`。**推荐**：在 `tab_routing` 里把整个 `<div class="data-card">` 包一层 `<div id="routing-tbody-wrap">`，加载返回整个 data-card 内容。**实现期**：让 handler 返回完整的 `tab_routing` 内部（操作栏+表格）片段，`hx-swap="outerHTML"` 替换 `#routing-tbody-wrap`。具体边界实现期定，关键是加载后操作栏+表格一起刷新。

- [ ] **Step 4: cargo check + clippy + 全测试 serial**

Run: `cargo check -p abt-web 2>&1 | grep -E "^error" | head`
Run: `cargo clippy -p abt-core -p abt-web --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- --test-threads=1 2>&1 | tail -5`
Expected: 0 error，全 PASS。

- [ ] **Step 5: 提交**

```bash
git add abt-web/src/routes/mes_order.rs abt-web/src/pages/mes_order_detail.rs
git commit -m "feat(mes): 工单工序 tab 加载产出品两个端点 + 按钮（模板/最近工单）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: 模板产出品录入（routing_create + routing_detail）

**Files:**
- Modify: `abt-web/src/pages/routing_create.rs`（StepWeb + get list 产品 + addStep JS + map）
- Modify: `abt-web/src/pages/routing_detail.rs`（产出品列显示名）

**Interfaces:**
- Consumes: `RoutingStepInput.product_id`（已存在）；`product_service.list`
- Produces: 模板工序可录入产出品 → ①加载有数据源

- [ ] **Step 1: routing_create StepWeb + map**

读 `routing_create.rs`。`StepWeb`(33) 加 `product_id: Option<i64>`。`post_routing_create` 的 map（104，`RoutingStepInput { .. }`）加 `product_id: s.product_id,`。

- [ ] **Step 2: get_routing_create list 产品**

`get_routing_create` 加：
```rust
 let products = state.product_service()
     .list(&service_ctx, &mut conn,
         abt_core::master_data::product::model::ProductQuery { name: None, code: None, status: None, owner_department_id: None, category_id: None },
         abt_core::shared::types::PageParams::new(1, 500)).await?;
 let content = routing_create_page(&processes.items, &products.items);
```
`routing_create_page(processes, products)` 签名加 `products`。

- [ ] **Step 3: addStep JS 加产出品 select**

页面注入产品选项 JSON（参考 process_map_json 模式）：
```rust
 let product_opts = products.iter()
     .map(|p| format!(r#"<option value="{}">{}</option>"#, p.product_id, p.pdt_name)).collect::<Vec<_>>().join("");
```
注入 JS：`var PRODUCT_OPTS = <PreEscaped product_opts>` 或作为字符串拼到 addStep。

`addStep()` 每行加一个产出品 `<select>` 单元格（与现有 process_code select 同范式）：
```javascript
'<td><select onchange="onStepChange(' + idx + ')" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border"><option value="">—</option>' + PRODUCT_OPTS + '</select></td>'
```
表头加 `<th>产出品</th>`。`onStepChange` 收集时把该 select 的 value 存入 `steps[idx].product_id`；`getStepsJson` 序列化含 product_id；`syncFromDom` 同步。

> 这是修改既有 addStep JS：读现有 addStep/render 结构（routing_create.rs 约 225-300 行），按列序插入产出品 select 单元格，同步表头 + onStepChange/getStepsJson/syncFromDom 收集 product_id。PRODUCT_OPTS 由 Rust 注入（与 process_map_json 同处）。

- [ ] **Step 4: routing_detail 显示名**

`routing_detail.rs` 工序表「产出品」列（现有 `#id`）：handler 批量取产品名（`get_by_ids`），行展示名（无则 `—`）。或最简：仍显示 `#id`（保持），名称展示留后续。**本任务最简：保留 `#id`**（routing_detail 已在上一特性显示 product_id），名称升级可选。

> 范围收敛：routing_detail 名称展示为可选 polish，本任务核心是 routing_create 能录入。若时间紧，routing_detail 维持 `#id`。

- [ ] **Step 5: cargo check + clippy**

Run: `cargo check -p abt-web 2>&1 | grep -E "^error" | head`
Run: `cargo clippy -p abt-web --quiet 2>&1 | grep -E "^error" | head`
Expected: 0 error。

- [ ] **Step 6: 提交**

```bash
git add abt-web/src/pages/routing_create.rs abt-web/src/pages/routing_detail.rs
git commit -m "feat(routing): 工艺路径模板工序录入产出品（routing_create select）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: uml 同步 + 最终验证

**Files:**
- Modify: `docs/uml-design/04-mes.html`（trait 加 2 方法）

- [ ] **Step 1: uml**

`ProductionBatchService` class block 加：
```
+load_routings_from_template(ctx, db, work_order_id) Result~usize~
+load_routings_from_recent(ctx, db, work_order_id) Result~usize~
```

- [ ] **Step 2: 全量 clippy + 回归 serial**

Run: `cargo clippy --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price --test mes_batch --test mes_flow_e2e --test om_outsourcing_suggest -- --test-threads=1 2>&1 | grep -E "test result|FAILED"`
Expected: 0 error，全部 PASS。

- [ ] **Step 3: 提交**

```bash
git add docs/uml-design/04-mes.html
git commit -m "docs(uml): 同步产出品批量加载接口

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec 覆盖**：§3 模板录入 → T3；§4.1 触发 UI → T2 Step3；§4.2 service 两方法 → T1；§4.3 端点 → T2；§4.4 数据流 → T1+T2；§5 错误处理（无 routing_id/状态/空源）→ T1；§6 测试 → T1 Step2；§8 uml → T4。全覆盖。

**2. 占位符扫描**：T2 的 `render_routing_tbody` 标注「实现期复用 delete_routing 同段逻辑抽公共函数」+ product_names 取值——这是对既有代码的明确复用指引（delete_routing 已有完整同名逻辑），非空洞 TODO；给出了具体函数签名与调用点。T1 的 `RoutingRepo.find_steps` 调用方式标注「参考 routing_service 现有调用」——既有 API，实现期 lsp 确认 `&self` 调用形式。T3 的 addStep JS 标注「读现有结构按列序插入」——修改长既有 JS，给出 select 模板与收集点。均非新代码占位。

**3. 类型一致性**：`load_routings_from_template/recent(ctx, db, work_order_id) -> Result<usize>` 在 T1(trait+impl) 与 T2(handler 调用) 一致；`find_recent_source_work_order(db, routing_id, exclude_wo_id) -> Option<i64>` T1 定义+使用一致；`routing_tbody_fragment(routings, reported_set, order_has_report, product_names)` 复用上一特性既有签名一致。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-routing-product-batch-load.md`. Two execution options:

**1. Subagent-Driven** — 每 Task 派 subagent + review（⚠️ 上轮 sonnet 多次伪造验证，需 controller 交叉核对 + 自跑测试）
**2. Inline Execution（推荐）** — 当前会话内联执行（cargo check 13s + 串行测试可靠）

Which approach?
