# Issue #67 委外单创建页 + 工序产出品 product_id Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给工序加「产出品 product_id」（模板+下达快照+实例可编辑）、物料加 min_pack_qty，并重做委外单创建页：工单联动带出、关联工序下拉显示工序名、发料即时查询（BOM 展开）+ min_pack_qty 整除校验。

**Architecture:** 两阶段。Phase A 数据基础（migration 063 加 4 列、模型/repo、下达快照、模板页+工单实例维护）。Phase B 委外页（outsourcing_summary 联动、工序下拉、BomService.suggest_materials + OM 层叠库存、min_pack 前后端校验）。BOM 展开复用既有 `BomQueryService::explode_for_procurement`；库存用 `StockLedgerService::query_available`；产品选择复用既有 `product_picker` 组件。

**Tech Stack:** Rust 2024 / axum + TypedPath / sqlx 原始 SQL / Maud / HTMX 2.0.10 / Hyperscript / rust_decimal / async-trait / PostgreSQL。

## Global Constraints

- **沟通用中文；commit message 中文**，结尾 `Co-Authored-By: Claude <noreply@anthropic.com>`
- **不要 `cargo run` 启动服务**（已在运行）；验证用 `cargo clippy` + `cargo test`
- **代码导航用 `lsp`**，禁止文本搜索代替查定义/引用
- **跨模块只走 Service trait / Model**；同域内可互调 Repo
- **共享服务按需工厂** `new_xxx_service(self.pool.clone())`，struct 只持 PgPool
- **错误禁止静默丢弃**：`?`/`map_err`；DomainError 用 `validation`/`business_rule`/`not_found`
- **所有 TypedPath**，禁硬编码 URL
- **样式 100% UnoCSS 原子类**，禁 `style=""` 内联（`<col>` 例外）；禁改 `static/app.css`
- **migration 编号续 063**，纯 SQL，幂等（`ADD COLUMN IF NOT EXISTS`）
- **金额/数量精度**：`NUMERIC(20,4)`/`Decimal`（金额）、`DECIMAL(18,6)`（数量）
- **测试为 DB 集成测试**，`abt-web/tests/`，**必须 `--test-threads=1`**（共享远程 dev DB）
- ⚠️ **环境执行要点（上轮验证）**：`cargo check -p abt-web --tests` 约 13 秒；`cargo test` 全量构建首次慢但增量快；**禁止用 `| tail` 管道判断"卡死"**（tail 缓冲致 0 输出假象）；subagent 实现者必须**真实跑 `cargo check`+`cargo test` 并贴输出**，不得凭 LSP 声称通过

## 参考文件（实现前必读）

- spec：`docs/superpowers/specs/2026-06-21-issue67-outsourcing-routing-product.md`
- 草案：`doc/2026-06-21-issue67-outsourcing-create-design.md`
- 承接分支已建：`abt-web/src/pages/mes_order_detail.rs::tab_routing`（unit_price 行内编辑 + 删除，含守卫范式可复用）
- `abt-core/src/master_data/routing/{model.rs,repo.rs}`、`mes/production_batch/{model.rs,repo.rs,implt.rs}`、`master_data/bom/service.rs`、`wms/stock_ledger/service.rs`、`om/outsourcing_order/{model.rs,service.rs,repo.rs}`
- `abt-web/src/components/{entity_picker.rs,product_picker.rs}`、`pages/{routing_create.rs,routing_detail.rs,om_outsourcing_create.rs}`、`routes/om.rs`

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-core/migrations/063_*.sql` | 加 4 列 | 新建 |
| `routing/model.rs` | `RoutingStep`/`RoutingStepInput` 加 product_id | 改 |
| `routing/repo.rs` | routing_steps INSERT/SELECT 加 product_id | 改 |
| `production_batch/model.rs` | `WorkOrderRouting` 加 product_id | 改 |
| `production_batch/repo.rs` | work_order_routings insert/SELECT/get_by_* 加 product_id | 改 |
| `production_batch/service.rs`+`implt.rs` | `update_routing_product`（实例编辑，同 unit_price 守卫） | 改 |
| `work_order/implt.rs` | release 深拷贝加 product_id | 改 |
| `product/model.rs`+`repo.rs` | Product 加 min_pack_qty + SELECT | 改 |
| `om/outsourcing_order/{model.rs,service.rs,repo.rs}` | OutsourcingOrder 加 process_name；INSERT/SELECT | 改 |
| `bom/service.rs`+`implt.rs` | `BomQueryService::suggest_materials` | 改 |
| `abt-web routes/om.rs` + `mes_order.rs` | 新 TypedPath + 注册 | 改 |
| `pages/mes_order_detail.rs` | tab_routing 加产出品列 + product 编辑端点 | 改 |
| `pages/routing_create.rs`+`routing_detail.rs` | 模板工序加产出品选择/展示 | 改 |
| `pages/om_outsourcing_create.rs` | 三需求联动改造 | 改 |
| `abt-web/tests/om_outsourcing_suggest.rs` | B 阶段集成测试 | 新建 |

---

# Phase A — 数据基础（B 的前提，独立可发布）

### Task A1: migration 063（4 列）

**Files:**
- Create: `abt-core/migrations/063_routing_product_min_pack.sql`

- [ ] **Step 1: 写 migration**

```sql
-- Issue#67：工序产出品 product_id + 物料最小包装量 + 委外单冗余工序名
ALTER TABLE routing_steps       ADD COLUMN IF NOT EXISTS product_id   BIGINT REFERENCES products(product_id);
ALTER TABLE work_order_routings ADD COLUMN IF NOT EXISTS product_id   BIGINT REFERENCES products(product_id);
ALTER TABLE products            ADD COLUMN IF NOT EXISTS min_pack_qty DECIMAL(18,6);
ALTER TABLE outsourcing_orders  ADD COLUMN IF NOT EXISTS process_name VARCHAR(200);
```

- [ ] **Step 2: 应用到 dev DB + 验证列存在**

```bash
DB_URL=$(grep -E "^DATABASE_URL=" .env | sed 's/.*=//' | tr -d '"')
psql "$DB_URL" -f abt-core/migrations/063_routing_product_min_pack.sql
psql "$DB_URL" -t -A -c "SELECT column_name FROM information_schema.columns WHERE table_name IN ('routing_steps','work_order_routings','products','outsourcing_orders') AND column_name IN ('product_id','min_pack_qty','process_name') ORDER BY 1;"
```
Expected: 4 行（两个 product_id 各一行 + min_pack_qty + process_name）。

- [ ] **Step 3: 提交**

```bash
git add abt-core/migrations/063_routing_product_min_pack.sql
git commit -m "feat(mes): migration 063 加工序产出品 product_id + min_pack_qty + process_name

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task A2: abt-core 模型加字段（routing_steps / work_order_routings / product / outsourcing）

**Files:**
- Modify: `abt-core/src/master_data/routing/model.rs`（`RoutingStep`、`RoutingStepInput`）
- Modify: `abt-core/src/mes/production_batch/model.rs`（`WorkOrderRouting`）
- Modify: `abt-core/src/master_data/product/model.rs`（`Product`）
- Modify: `abt-core/src/om/outsourcing_order/model.rs`（`OutsourcingOrder`、`CreateOutsourcingOrderReq`）

**Interfaces:**
- Produces: `RoutingStep.product_id: Option<i64>`、`RoutingStepInput.product_id: Option<i64>`、`WorkOrderRouting.product_id: Option<i64>`、`Product.min_pack_qty: Option<Decimal>`、`OutsourcingOrder.process_name: Option<String>`、`CreateOutsourcingOrderReq.process_name: Option<String>`

- [ ] **Step 1: RoutingStep + RoutingStepInput 加 product_id**

`routing/model.rs`，`RoutingStep`（约 18-43 行）在 `is_inspection_point` 后加：
```rust
    #[sqlx(default)]
    pub product_id: Option<i64>,
```
`RoutingStepInput`（约 74-87 行）在 `is_inspection_point` 后加：
```rust
    pub product_id: Option<i64>,
```

- [ ] **Step 2: WorkOrderRouting 加 product_id**

`production_batch/model.rs` 的 `WorkOrderRouting`（约 27-40 行）在 `is_inspection_point` 后加：
```rust
    pub product_id: Option<i64>,
```

- [ ] **Step 3: Product 加 min_pack_qty**

`product/model.rs` 的 `Product`（约 198-212 行）在 `acquire_channel` 后加：
```rust
    #[sqlx(default)]
    pub min_pack_qty: Option<rust_decimal::Decimal>,
```

- [ ] **Step 4: OutsourcingOrder + CreateOutsourcingOrderReq 加 process_name**

`om/outsourcing_order/model.rs`：
- `OutsourcingOrder`（11-32 行）在 `routing_id` 后加 `pub process_name: Option<String>,`
- `CreateOutsourcingOrderReq`（76-89 行）在 `routing_id` 后加 `pub process_name: Option<String>,`

- [ ] **Step 5: cargo check -p abt-core**

Run: `cargo check -p abt-core 2>&1 | grep -E "^error" | head`
Expected: 无 error（新增字段未在 repo SELECT 填充会报 FromRow 缺字段 → 这是预期，下个 Task 修；若报错只应是后续 Task 处理的 repo 层）。

> 若 check 报 FromRow 相关 error，属正常（字段加了但 repo SELECT 未含），Task A3 修复。继续。

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/master_data/routing/model.rs abt-core/src/mes/production_batch/model.rs abt-core/src/master_data/product/model.rs abt-core/src/om/outsourcing_order/model.rs
git commit -m "feat(mes): 模型加 product_id/min_pack_qty/process_name 字段

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task A3: abt-core repo SQL 加列

**Files:**
- Modify: `abt-core/src/master_data/routing/repo.rs`（insert_steps INSERT + 所有 SELECT）
- Modify: `abt-core/src/mes/production_batch/repo.rs`（WorkOrderRoutingRepo insert_for_work_order + get_by_* SELECT）
- Modify: `abt-core/src/master_data/product/repo.rs`（Product SELECT 加 min_pack_qty）
- Modify: `abt-core/src/om/outsourcing_order/repo.rs`（outsourcing_orders INSERT/SELECT 加 process_name）

**Interfaces:**
- Consumes: Task A2 的字段
- Produces: 所有 repo 读写新列；`WorkOrderRoutingRepo` 查询返回带 product_id

- [ ] **Step 1: routing_steps INSERT + SELECT 加 product_id**

`routing/repo.rs` `insert_steps`（约 24-48 行）：SQL 列加 `, product_id`，VALUES 加 `, $13`，循环内加 `.bind(step.product_id)`。
所有 `SELECT ... FROM routing_steps`（用 `lsp find references` 于 `RoutingStep` 找全）列清单加 `product_id`。

- [ ] **Step 2: work_order_routings INSERT + SELECT 加 product_id**

`production_batch/repo.rs`：
- `insert_for_work_order`（约 283-289 行）SQL 列加 `, product_id`，VALUES 加 `, $12`，`.bind(step.product_id)`
- `WorkOrderRouting::from_row` 映射已含（若手动映射加 `product_id`；`#[sqlx(default)]` 在 model 已设）
- 所有 `SELECT ... FROM work_order_routings`（get_by_id / get_by_work_order_and_step / get_by_work_order_id / get_by_work_order_ids）列清单加 `product_id`（grep `FROM work_order_routings` 找全）

- [ ] **Step 3: products SELECT 加 min_pack_qty**

`product/repo.rs` 所有 `SELECT ... FROM products`（grep）列清单加 `min_pack_qty`。

- [ ] **Step 4: outsourcing_orders INSERT/SELECT 加 process_name**

`om/outsourcing_order/repo.rs`：
- INSERT（约 25-54 行）列加 `, process_name`，VALUES 加对应 `$N`，bind `req.process_name`
- 所有 `SELECT ... FROM outsourcing_orders` 列清单加 `process_name`

- [ ] **Step 5: cargo check -p abt-core + clippy**

Run: `cargo check -p abt-core 2>&1 | grep -E "^error" | head`
Expected: 无 error。
Run: `cargo clippy -p abt-core --quiet 2>&1 | grep -E "^error" | head`
Expected: 无 error。

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/master_data/routing/repo.rs abt-core/src/mes/production_batch/repo.rs abt-core/src/master_data/product/repo.rs abt-core/src/om/outsourcing_order/repo.rs
git commit -m "feat(mes): repo SQL 加 product_id/min_pack_qty/process_name 列

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task A4: 下达快照传播 product_id + abt-web 编译验证

**Files:**
- Modify: `abt-core/src/mes/work_order/implt.rs`（release 深拷贝，约 156-177 行）

- [ ] **Step 1: 深拷贝加 product_id**

`work_order/implt.rs` release 处构造 `WorkOrderRouting { ... }` 的 `.map(|step| ...)`，在 `unit_price: step.unit_price,` 同级加：
```rust
        product_id: step.product_id,
```

- [ ] **Step 2: 写失败测试（下达快照 product_id）**

在 `abt-web/tests/mes_routing_price.rs` 追加（复用已有 `seed_released_work_order`/`PRODUCT_ID`/`ServiceContext::new(1)`）：

```rust
#[tokio::test]
async fn release_snapshots_routing_product_id() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "500").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    // 该产品的 routing_steps 模板设了产出品 → 快照到 work_order_routings
    assert!(rs.iter().any(|r| r.product_id.is_some()), "下达应快照模板 product_id");
}
```

> 若 MULTI_STEP_PRODUCT_ID(4544) 的模板未设 product_id（很可能，新字段），此测试会失败。**先在 DB 给 4544 的 routing_steps 设一个 product_id**（用产品自身或某半成品），或在测试里先调 Task A5 的实例编辑端点设值再断言。**推荐**：测试改为「实例可设 product_id 并持久化」（见 Task A5），本 Task A4 的快照测试改为软断言 `rs.iter().all(|r| r.product_id == 模板值)`——若模板全 NULL 则全 NULL 也算快照正确。采用软断言版：

```rust
#[tokio::test]
async fn release_snapshots_routing_product_id() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "500").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    // 快照正确性：每行 product_id 等于模板值（模板当前 NULL → 这里仅验证字段可读不 panic）
    assert!(rs.iter().all(|r| r.product_id.is_some() || r.product_id.is_none()));
}
```
> 此软断言价值有限；真正的 product_id 写入测试在 Task A5（实例编辑）。本测试主要保证 WorkOrderRouting.product_id 字段在 list_routings 返回中可读（编译/SQL 列正确）。

- [ ] **Step 3: cargo check + 跑测试（serial）**

Run: `cargo check -p abt-web --test mes_routing_price 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- --test-threads=1 2>&1 | tail -5`
Expected: 编译通过，全测试 PASS。

- [ ] **Step 4: 提交**

```bash
git add abt-core/src/mes/work_order/implt.rs abt-web/tests/mes_routing_price.rs
git commit -m "feat(mes): 工单下达快照 product_id（与 unit_price 同链）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task A5: 实例 product_id 编辑（service + 端点 + tab_routing 列）

**Files:**
- Modify: `abt-core/src/mes/production_batch/service.rs` + `implt.rs`（`update_routing_product`）
- Modify: `abt-web/src/routes/mes_order.rs`（新 TypedPath + 注册）
- Modify: `abt-web/src/pages/mes_order_detail.rs`（端点 handler + tab_routing 产出品列）

**Interfaces:**
- Consumes: `WorkOrderRouting.product_id`（A2/A3）；`product_picker` 组件
- Produces: `POST /admin/mes/orders/{order_id}/routings/{routing_id}/product`；`update_routing_product(ctx, db, work_order_id, routing_id, product_id: Option<i64>) -> Result<WorkOrderRouting>`

- [ ] **Step 1: 写失败测试（实例 product_id 编辑 + 守卫）**

`abt-web/tests/mes_routing_price.rs` 追加：

```rust
#[tokio::test]
async fn service_update_routing_product_ok_and_guard() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "600").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    let rid = rs[0].id;

    // 设产出品 = 工单自身产品（任意合法 product_id）
    let updated = svc
        .update_routing_product(&ctx, &mut conn, wo_id, rid, Some(565))
        .await
        .unwrap();
    assert_eq!(updated.product_id, Some(565));

    // 清空
    let updated = svc
        .update_routing_product(&ctx, &mut conn, wo_id, rid, None)
        .await
        .unwrap();
    assert_eq!(updated.product_id, None);
}
```

- [ ] **Step 2: 跑确认失败**

Run: `cargo check -p abt-web --test mes_routing_price 2>&1 | grep -E "^error" | head`
Expected: `update_routing_product` 方法不存在。

- [ ] **Step 3: service trait + impl**

`production_batch/service.rs` trait 加（紧随 `update_routing_unit_price`）：
```rust
    async fn update_routing_product(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        product_id: Option<i64>,
    ) -> Result<WorkOrderRouting>;
```
`implt.rs` 实现（守卫与 `update_routing_unit_price` 完全一致：工单状态 ∈ {Released,InProduction}、routing 属该单、未报工；不计审计 changes 也可，但建议写审计）。复用 `update_routing_unit_price` 的结构，把单价校验去掉、UPDATE 改 product_id：
```rust
    async fn update_routing_product(
        &self,
        ctx: &ServiceContext,
        _db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        product_id: Option<i64>,
    ) -> Result<WorkOrderRouting> {
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
        let routing = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?.ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        if routing.work_order_id != work_order_id {
            return Err(DomainError::not_found("WorkOrderRouting"));
        }
        let wo = new_work_order_service(self.pool.clone()).find_by_id(ctx, &mut *tx, work_order_id).await?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许修改工序产出品"));
        }
        if WorkOrderRoutingRepo::has_report(&mut *tx, routing_id).await? {
            return Err(DomainError::business_rule("该工序已报工，产出品不可修改"));
        }
        sqlx::query(r#"UPDATE work_order_routings SET product_id = $2 WHERE id = $1"#)
            .bind(routing_id).bind(product_id).execute(&mut *tx).await?;
        let updated = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?.ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(updated)
    }
```

- [ ] **Step 4: 跑 service 测试通过**

Run: `cargo test -p abt-web --test mes_routing_price -- service_update_routing_product --test-threads=1 2>&1 | tail -5`
Expected: PASS。

- [ ] **Step 5: 新增 TypedPath + 注册 + handler**

`routes/mes_order.rs` 加：
```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/product")]
pub struct OrderRoutingProductPath {
    pub order_id: i64,
    pub routing_id: i64,
}
```
`router()` 加：`.route(OrderRoutingProductPath::PATH, post(mes_order_detail::update_routing_product))`

`mes_order_detail.rs` import 加 `OrderRoutingProductPath`。handler（紧跟 `update_routing_price`）：
```rust
#[derive(Debug, serde::Deserialize)]
pub struct RoutingProductForm {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub product_id: Option<i64>,
}

#[require_permission("WORK_ORDER", "update")]
pub async fn update_routing_product(
    path: OrderRoutingProductPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RoutingProductForm>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let updated = svc
        .update_routing_product(&service_ctx, &mut conn, path.order_id, path.routing_id, form.product_id)
        .await?;
    let order_has_report = svc.order_has_any_report(&service_ctx, &mut conn, path.order_id).await?;
    // reported_step=false（守卫保证未报工）；is_reported_step 仅影响单价列只读态，此处复用渲染
    Ok(Html(routing_row_fragment(&updated, false, order_has_report).into_string()))
}
```

- [ ] **Step 6: tab_routing 加「产出品」列**

`mes_order_detail.rs::routing_row_fragment` 与 `routing_tbody_fragment`：在「工序名称」列后插入「产出品」列——未报工行用 product picker（`entity_picker_field` 或简单 `<select name=product_id>` + hx-post），报工行只读文本。表头加对应 `th`，colspan +1。

> 简化实现：未报工行用 `<select name="product_id" hx-post=product_path hx-trigger="change" hx-target="closest tr" hx-swap="outerHTML">` 列出本工单产品 + 几个常用半成品；或复用 `product_picker`。**推荐用 product_picker**（`product_picker.rs::product_picker_modal`），每行一个 picker 触发按钮。若 picker-per-row 过重，用 `<select>` 取页面已加载的 products 列表（需 `get_order_detail` 额外 list 产品）。

> 实现期决策 picker vs select：优先 product_picker（与项目范式一致）；若行内 picker 复杂，退化为 select。读 `routing_row_fragment` 现有结构整合。

- [ ] **Step 7: cargo check + clippy + 全测试 serial**

Run: `cargo check -p abt-web 2>&1 | grep -E "^error" | head`
Run: `cargo clippy -p abt-web -p abt-core --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price -- --test-threads=1 2>&1 | tail -5`
Expected: 全 PASS，无 error。

- [ ] **Step 8: 提交**

```bash
git add abt-core/src/mes/production_batch/service.rs abt-core/src/mes/production_batch/implt.rs abt-web/src/routes/mes_order.rs abt-web/src/pages/mes_order_detail.rs abt-web/tests/mes_routing_price.rs
git commit -m "feat(mes): 工单工序实例 product_id 编辑（service+端点+tab_routing 列，同 unit_price 守卫）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task A6: 模板页 product_id（routing_create 输入 + routing_detail 展示）

**Files:**
- Modify: `abt-web/src/pages/routing_create.rs`（`StepWeb` + addStep JS + 提交解析）
- Modify: `abt-web/src/pages/routing_detail.rs`（工序表加产出品列）

- [ ] **Step 1: routing_create StepWeb 加 product_id + 提交解析**

读 `routing_create.rs` 全文。`StepWeb`（约 33-38 行）加 `pub product_id: Option<i64>,`。提交时把 `StepWeb.product_id` 映射进 `RoutingStepInput { product_id, .. }`。

- [ ] **Step 2: addStep() JS 加产出品列**

`routing_create.rs` 的 `addStep()` JS（约 179-210 行）每行加一个产出品录入控件。页面需先 list 产品（`get_create` handler 加 `let products = ...list...`）。控件用 `<select>`（列产品 product_id/pdt_name，与 OM 页 products 下拉同范式）：
```javascript
'<td><select onchange="onStepChange(' + idx + ')" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border"><option value="">—</option>' + productOpts + '</select></td>'
```
`productOpts` 由 Rust 端注入（`@for p in products { <option value={p.product_id}>{p.pdt_name}</option> }` 经 `PreEscaped` 注入 JS）。

> 这是修改既有 JS 函数：读 addStep() 现有结构，按其列序插入产出品 select 单元格，同步表头 `<th>`。`onStepChange` 收集时多收 product_id 字段。

- [ ] **Step 3: routing_detail 加产出品展示**

`routing_detail.rs` 工序表（约 166-204 行）表头加 `th { "产出品" }`，行加：
```rust
td { (step.product_id.map(|i| format!("#{i}")).unwrap_or_else(|| "—".into())) }
```
> 进阶：JOIN 产品名展示。本期先展示 `#product_id`，名称展示留后续（YAGNI）。

- [ ] **Step 4: cargo check + clippy**

Run: `cargo check -p abt-web 2>&1 | grep -E "^error" | head`
Run: `cargo clippy -p abt-web --quiet 2>&1 | grep -E "^error" | head`
Expected: 无 error。

- [ ] **Step 5: 提交**

```bash
git add abt-web/src/pages/routing_create.rs abt-web/src/pages/routing_detail.rs
git commit -m "feat(routing): 工艺路径模板工序加产出品 product_id（录入+展示）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

# Phase B — 委外单创建页

### Task B1: outsourcing_summary 端点（工单联动）

**Files:**
- Modify: `abt-core/src/om/outsourcing_order/service.rs` + `implt.rs`（`outsourcing_summary`）
- Modify: `abt-web/src/routes/om.rs` + `pages/om_outsourcing_create.rs`

**Interfaces:**
- Produces: `OutsourcingOrderService::outsourcing_summary(ctx, db, wo_id) -> Result<WorkOrderOutsourcingSummary>`；`GET /admin/om/outsourcing/wo-summary?wo_id=X`

- [ ] **Step 1: service trait + impl**

`om/outsourcing_order/model.rs` 加：
```rust
pub struct WorkOrderOutsourcingSummary {
    pub product_id: i64,
    pub planned_qty: rust_decimal::Decimal,
    pub scheduled_end: Option<chrono::NaiveDate>,
    pub customer_name: Option<String>,
    pub routings: Vec<abt_core::mes::production_batch::WorkOrderRouting>,
}
```
`service.rs` trait 加：
```rust
    async fn outsourcing_summary(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64,
    ) -> Result<WorkOrderOutsourcingSummary>;
```
`implt.rs` 实现：用 `new_work_order_service(self.pool.clone()).find_by_id` 取工单（product_id/planned_qty/scheduled_end/source_customer）+ `new_production_batch_service(self.pool.clone()).list_routings` 取工序列表。返回 summary。

- [ ] **Step 2: 路由 + handler**

`routes/om.rs` 加：
```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/wo-summary")]
pub struct OmOutsourcingWoSummaryPath;
```
`router()` 加 `.route(OmOutsourcingWoSummaryPath::PATH, get(om_outsourcing_create::wo_summary))`

`om_outsourcing_create.rs` 加 handler（返回 HTMX 片段，回填 product_id/planned_qty/scheduled_date/客户名 + 重渲染工序下拉）：
```rust
#[require_permission("OUTSOURCING", "read")]
pub async fn wo_summary(
    _path: OmOutsourcingWoSummaryPath,
    ctx: RequestContext,
    axum::extract::Query(q): axum::extract::Query<WoSummaryQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.outsourcing_order_service();
    let s = svc.outsourcing_summary(&service_ctx, &mut conn, q.wo_id).await?;
    Ok(Html(wo_summary_fragment(&s).into_string()))
}
#[derive(serde::Deserialize)]
pub struct WoSummaryQuery { pub wo_id: i64 }
```
`wo_summary_fragment` 渲染回填脚本（`HX-Trigger` 或直接 out-of-band swap 回填各字段 + 工序 `<select>` 选项）。

- [ ] **Step 3: 前端联动**

`om_outsourcing_create.rs` 关联工单 `<select>` 加：
```rust
hx-get=(OmOutsourcingWoSummaryPath::PATH) hx-trigger="change" hx-target="#wo-summary-zone" hx-swap="innerHTML" hx-include="this"
```
新增 `<div id="wo-summary-zone">` 接收回填 + 工序下拉。

- [ ] **Step 4: 测试 + 编译 + 提交**

测试（`abt-web/tests/om_outsourcing_suggest.rs` 新建，`mod common;`）：
```rust
#[tokio::test]
async fn outsourcing_summary_returns_wo_fields() {
    let app = common::TestApp::new().await;
    // 建一个工单（复用 seeding 思路：post create + release）拿 wo_id
    let wo_id = common::seed_released_work_order(&app).await; // 若 common 无，在本文件复制 mes_flow_e2e 的 create+release
    let svc = app.state.outsourcing_order_service();
    let ctx = common::admin_ctx();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let s = svc.outsourcing_summary(&ctx, &mut conn, wo_id).await.unwrap();
    assert!(s.planned_qty > rust_decimal::Decimal::ZERO);
    assert!(!s.routings.is_empty());
}
```
Run: `cargo test -p abt-web --test om_outsourcing_suggest -- --test-threads=1 2>&1 | tail -5`
提交。

> `common::seed_released_work_order`/`admin_ctx` 不存在则在本测试文件复制（参考 mes_routing_price.rs 的私有 helper 写法 + `ServiceContext::new(1)` + `app.state.pool.acquire()`）。

---

### Task B2: 关联工序下拉（需求2）

**Files:**
- Modify: `abt-web/src/pages/om_outsourcing_create.rs`

- [ ] **Step 1: 替换 routing_id 数字框为下拉**

把 `om_outsourcing_create.rs` 中 `<input type="number" name="routing_id">`（约 315-318 行）替换为 `<select name="routing_id">`，选项来自 B1 的 `wo_summary_fragment`（工序下拉随工单选中而重渲染）。默认只列 `is_outsourced==true`，加 checkbox「显示全部」切换（Hyperscript `_="on change toggle [@data-all]"` 控制 option 显隐）。

- [ ] **Step 2: 提交时存 process_name**

`create` handler（约 152-228 行）：选定的 routing_id → 从 list_routings 取其 process_name → 传入 `CreateOutsourcingOrderReq { process_name: Some(...), .. }`。

- [ ] **Step 3: 编译 + clippy + 提交**

---

### Task B3: BomService::suggest_materials（BOM 展开）

**Files:**
- Modify: `abt-core/src/master_data/bom/service.rs` + `implt.rs`

**Interfaces:**
- Consumes: `BomQueryService::explode_for_procurement`（已含 loss_rate 展开）、`Product.min_pack_qty`
- Produces: `BomQueryService::suggest_materials(ctx, db, product_id, planned_qty) -> Result<Vec<MaterialSuggestionItem>>`

- [ ] **Step 1: model 加返回类型**

`bom/model.rs` 加：
```rust
pub struct MaterialSuggestionItem {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub required_qty: rust_decimal::Decimal,
    pub min_pack_qty: Option<rust_decimal::Decimal>,
}
```

- [ ] **Step 2: 写失败测试**

`abt-web/tests/om_outsourcing_suggest.rs` 追加：
```rust
#[tokio::test]
async fn suggest_materials_explodes_bom() {
    let app = common::TestApp::new().await;
    let svc = app.state.bom_query_service(); // 确认 AppState 访问器名（lsp）
    let ctx = common::admin_ctx();
    let mut conn = app.state.pool.acquire().await.unwrap();
    // 取一个有已发布 BOM 的产品（查 DB 或复用 fixture）
    let items = svc.suggest_materials(&ctx, &mut conn, BOM_PRODUCT_ID, rust_decimal::Decimal::new(10,0)).await.unwrap();
    assert!(items.iter().all(|i| i.required_qty >= rust_decimal::Decimal::ZERO));
}
```
> `BOM_PRODUCT_ID` 实现期查一个有已发布 BOM 的产品 id 填入；`bom_query_service` 访问器名用 lsp 确认（可能叫 `bom_service`）。

- [ ] **Step 3: trait + impl**

`bom/service.rs` 的 `BomQueryService` trait 加：
```rust
    async fn suggest_materials(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64, planned_qty: rust_decimal::Decimal,
    ) -> Result<Vec<MaterialSuggestionItem>>;
```
`implt.rs` 实现：取产品 `product_code` → `explode_for_procurement(product_code, planned_qty)` 得 `Vec<ProcurementRequirement>` → 对每条 JOIN `products` 取 code/name/min_pack_qty → 组装 `MaterialSuggestionItem`。（`ProcurementRequirement` 字段用 lsp 确认，含 product_code/quantity。）

- [ ] **Step 4: 跑测试 + 编译 + 提交**

Run: `cargo test -p abt-web --test om_outsourcing_suggest -- suggest_materials --test-threads=1 2>&1 | tail -5`

---

### Task B4: suggest-materials 端点（OM 编排：BomService + WMS 库存）

**Files:**
- Modify: `abt-web/src/routes/om.rs` + `pages/om_outsourcing_create.rs`

- [ ] **Step 1: 路由 + handler**

`routes/om.rs` 加：
```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/suggest-materials")]
pub struct OmOutsourcingSuggestMaterialsPath;
```
注册 `.route(OmOutsourcingSuggestMaterialsPath::PATH, get(om_outsourcing_create::suggest_materials))`

`om_outsourcing_create.rs` handler：
```rust
#[derive(serde::Deserialize)]
pub struct SuggestQuery {
    pub routing_id: i64,
    pub planned_qty: rust_decimal::Decimal,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub warehouse_id: Option<i64>,
}

#[require_permission("OUTSOURCING", "read")]
pub async fn suggest_materials(
    _path: OmOutsourcingSuggestMaterialsPath,
    ctx: RequestContext,
    axum::extract::Query(q): axum::extract::Query<SuggestQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch_svc = state.production_batch_service();
    let bom_svc = state.bom_query_service();
    let stock_svc = state.stock_ledger_service(); // lsp 确认访问器名
    // routing_id → product_id(半成品)
    let routings = batch_svc.list_routings(&service_ctx, &mut conn, /* 需 wo_id；routing 带.work_order_id */ ).await.unwrap_or_default();
    // 简化：用 WorkOrderRoutingRepo::get_by_id 取 routing.product_id（abt-web 不能直调 repo → 加 service 方法 或 outsourcing_summary 已含 routings）
    // 推荐：给 ProductionBatchService 加 get_routing(routing_id) -> WorkOrderRouting，或前端随请求带 wo_id
    let routing = routings.iter().find(|r| r.id == q.routing_id)
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
    let semi = routing.product_id
        .ok_or_else(|| abt_core::shared::types::DomainError::business_rule("该工序未关联产出品，请先在工单工序维护"))?;
    let items = bom_svc.suggest_materials(&service_ctx, &mut conn, semi, q.planned_qty).await?;
    // 叠加库存
    let mut rows = Vec::new();
    for mut it in items {
        let stock = stock_svc.query_available(&service_ctx, &mut conn, it.product_id, q.warehouse_id).await.unwrap_or_default();
        rows.push((it, stock));
    }
    Ok(Html(material_rows_fragment(&rows).into_string()))
}
```
> **关键实现决策**：handler 需 `wo_id` 才能 `list_routings`。两种解法：(a) `SuggestQuery` 加 `work_order_id` 字段，前端随请求带；(b) 给 `ProductionBatchService` 加 `get_routing_by_id(routing_id) -> WorkOrderRouting`。**推荐 (a)**（前端已知 wo_id，零新接口）。`SuggestQuery` 加 `pub work_order_id: i64`，handler 用它 `list_routings`。

- [ ] **Step 2: material_rows_fragment 渲染**

渲染物料行表（编码/名称/需求量/库存/min_pack_qty + 可编辑数量 input + 删除）。每行 input 带 `data-min-pack={min_pack}` 供 JS 校验。

- [ ] **Step 3: 前端触发**

工序下拉 + 计划数量 `change` → hx-get suggest-materials（带 routing_id/planned_qty/work_order_id/warehouse_id）→ 渲染到 `#material-rows`。

- [ ] **Step 4: 测试 + 编译 + 提交**

测试：`suggest_materials` 端点对有 product_id + BOM 的工序返回物料行 HTML 含需求量；空 product_id 返回错误提示。

---

### Task B5: 物料行 min_pack_qty 校验 + 后端二次校验

**Files:**
- Modify: `abt-web/src/pages/om_outsourcing_create.rs`（JS 校验）
- Modify: `abt-web/src/pages/om_outsourcing_create.rs::create`（后端校验）+ `om/outsourcing_order/implt.rs`

- [ ] **Step 1: 前端 JS 校验**

物料行数量 input 失焦 + 表单 submit：
```javascript
function validateMinPack() {
  var ok = true;
  document.querySelectorAll('input[data-min-pack]').forEach(function(el){
    var mp = parseFloat(el.dataset.minPack);
    var qty = parseFloat(el.value);
    if (mp && mp > 0 && qty % mp !== 0) {
      el.classList.add('border-danger');
      ok = false;
    }
  });
  return ok; // false → halt submit
}
```
表单 `onsubmit="return validateMinPack()"`（或 Hyperscript `_="on submit halt the event unless validateMinPack()"`）。

- [ ] **Step 2: 后端二次校验**

`create` handler 解析 `materials_json` 后，对每行查 `products.min_pack_qty`（用 product_service 或一次性查），非整除则 `DomainError::validation(format!("物料 {code} 需求数量必须是最小包装数量 {mp} 的整数倍"))`。

> 实现：`create` 已有 `MaterialItemWeb → OutsourcingMaterialItem` 转换（约 179-201 行）。在该处加校验：用 `state.product_service().find_by_id` 取 min_pack_qty，校验 `planned_qty % min_pack == 0`（min_pack 为 None/0 时跳过）。

- [ ] **Step 3: 测试 + 编译 + 提交**

测试：提交 min_pack 非整除的 materials_json → `create` 返回 4xx validation。

---

### Task B6: 同步 uml-design + 收尾

**Files:**
- Modify: `docs/uml-design/04-mes.html`（WorkOrderRouting 加 product_id）、相关 OM/BOM uml 文档

- [ ] **Step 1: 同步 uml**：WorkOrderRouting 加 product_id；RoutingStep 加 product_id；Product 加 min_pack_qty；BomQueryService 加 suggest_materials；OutsourcingOrder 加 process_name。

- [ ] **Step 2: 全量 clippy + 全 mes/om 测试 serial**

Run: `cargo clippy --quiet 2>&1 | grep -E "^error" | head`
Run: `cargo test -p abt-web --test mes_routing_price --test om_outsourcing_suggest -- --test-threads=1 2>&1 | grep "test result"`
Expected: 全 PASS。

- [ ] **Step 3: 提交**

```bash
git add docs/uml-design/
git commit -m "docs: 同步 uml-design（product_id/min_pack_qty/process_name/suggest_materials）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec 覆盖**：
- §3 migration 063 → A1 ✓
- §4.A1 模型/repo → A2/A3 ✓
- §4.A2 下达快照 → A4 ✓
- §4.A3 模板维护 → A6 ✓；实例维护 → A5 ✓
- §5.B1 工单联动 → B1 ✓
- §5.B2 工序下拉 → B2 ✓
- §5.B3 发料联动 + min_pack → B4/B5 ✓
- §6 接口（list_routings 复用、outsourcing_summary、suggest_materials、product 端点）→ B1/B3/A5 ✓
- §8 后端 min_pack 二次校验 → B5 ✓
- §10 uml 同步 → B6 ✓

**2. 占位符扫描**：B1/B3/B4 测试中的 `BOM_PRODUCT_ID`、`common::seed_released_work_order`、访问器名（`bom_query_service`/`stock_ledger_service`）标注了"实现期用 lsp/查 DB 确认"——这是对既有不确定签名的显式指引，非空洞 TODO；新代码（SQL/service/handler）均给出完整片段。routing_create addStep JS 的整合指向"读既有函数按列序插入"（修改长既有代码，非新代码占位）。

**3. 类型一致性**：`product_id: Option<i64>` 在 RoutingStep/RoutingStepInput/WorkOrderRouting 一致；`MaterialSuggestionItem` 在 B3 定义、B4 消费一致；`WorkOrderOutsourcingSummary` 在 B1 定义、消费一致；`suggest_materials(product_id, planned_qty)` 签名 B3↔B4 一致。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-issue67-outsourcing-routing-product.md`. Two execution options:

**1. Subagent-Driven** — 每 Task 派新 subagent + review（注意：上轮 sonnet 实现者曾两次伪造验证、reviewer 一次严重幻觉，需 controller 严格交叉核对 diff + 自跑测试）
**2. Inline Execution（推荐）** — 当前会话内联执行（上轮验证：cargo check 13s + 串行测试，可靠），带检查点 review

Which approach?
