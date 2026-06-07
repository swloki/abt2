# 流转卡查询实现计划

**路径**: `/admin/mes/cards`
**复杂度**: ★☆☆（最简单，改动最少）

## Context

当前 `mes_card_query.rs` 是一个 stub，有输入框但无 form 提交机制、无 HTMX 搜索、无结果展示。原型 `04-card-query.html` 定义了完整的搜索+结果展示功能。

## 原型设计要点

- 搜索区：输入流转卡序列号（支持扫码/手动输入），回车或点击查询按钮触发搜索
- 结果区：卡片信息网格（卡号、批次号、工单号、产品、数量、已完成、状态）
- 流程进度：水平步骤条显示各工序完成情况（如 SMT → DIP → 组装 → 测试）
- 报工记录：该卡关联的报工明细表
- 最近查询：最近查询过的流转卡列表

## 实现步骤

### Step 1: 后端 — 添加 `find_by_card_sn` 到 ProductionBatch 模块

**`abt-core/src/mes/production_batch/repo.rs`** — 新增方法：
```rust
pub async fn find_by_card_sn(
    executor: &mut PgConnection,
    card_sn: &str,
) -> Result<Option<ProductionBatch>>
```
SQL: `SELECT ... FROM production_batches WHERE card_sn = $1`

**`abt-core/src/mes/production_batch/service.rs`** — trait 新增：
```rust
async fn find_by_card_sn(&self, ctx: &ServiceContext, db: PgExecutor<'_>, card_sn: String) -> Result<Option<ProductionBatch>>;
```

**`abt-core/src/mes/production_batch/implt.rs`** — 实现：调用 `ProductionBatchRepo::find_by_card_sn`

### Step 2: 后端 — 添加 Card Query 查询路由

**`abt-web/src/routes/mes_batch.rs`** — 新增 TypedPath：
```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/cards/search")]
pub struct CardQuerySearchPath;
```
路由注册：`.route(CardQuerySearchPath::PATH, get(mes_card_query::search_card))`

### Step 3: 前端 — 重写 `mes_card_query.rs`

完整重写，包含：

1. **GET `/admin/mes/cards`** — 初始页面
   - 搜索区：`form` with `hx-get="/admin/mes/cards/search"` + `hx-target="#card-result"` + `hx-trigger="submit"` + input `name="q"`
   - 结果区：空 div `#card-result`

2. **GET `/admin/mes/cards/search?q=xxx`** — HTMX 搜索结果片段
   - 调用 `state.production_batch_service().find_by_card_sn()` 查找批次
   - 若找到：
     - 调用 `svc.get_product_name()` 获取产品名
     - 调用 `svc.list_routings()` 获取工序列表
     - 调用 `state.work_report_service().list_by_batch()` 获取报工记录
     - 渲染：信息网格 + 工序进度条 + 报工明细表
   - 若未找到：显示 "未找到该流转卡" 提示

### Step 4: 工序进度条 UI

使用 `workflow-steps` + `wf-step` CSS 类（已在 uno.config.ts 中定义）：
- 已完成工序：绿色圆点 ✓
- 当前工序：蓝色高亮 + 闪烁
- 未到达工序：灰色

### 涉及文件

| 文件 | 改动 |
|------|------|
| `abt-core/src/mes/production_batch/repo.rs` | 新增 `find_by_card_sn` |
| `abt-core/src/mes/production_batch/service.rs` | trait 新增 `find_by_card_sn` |
| `abt-core/src/mes/production_batch/implt.rs` | 实现 `find_by_card_sn` |
| `abt-web/src/routes/mes_batch.rs` | 新增 `CardQuerySearchPath` + 路由 |
| `abt-web/src/pages/mes_card_query.rs` | 完整重写 |
