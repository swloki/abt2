# 排程看板实现计划

**路径**: `/admin/mes/schedule`
**复杂度**: ★★☆

## Context

当前 `mes_schedule_board.rs` 是 stub。原型 `04-schedule-board.html` 定义了看板视图（4列卡片）+ 统计行。不需要新建数据库表，所有数据来源于现有的 `production_batches` + `work_orders` + `work_order_routings`。

## 原型设计要点

- **统计行**：活跃工单数、进行中批次、已完成批次、延期风险、待排产
- **看板 4 列**：
  - 待排产（Pending, status=1）
  - 进行中（InProgress, status=2 + Suspended status=3）
  - 待入库（PendingReceipt, status=4）
  - 已完成（Completed, status=5）
- **卡片内容**：批次号、产品名、计划/完成数量、进度条、标签（MTO/MTS、延期风险）
- 排除 `Cancelled`（status=6）的批次

## 实现步骤

### Step 1: 后端 — Dashboard 添加看板查询

**`abt-core/src/mes/dashboard/model.rs`** — 新增模型：

```rust
/// 看板统计
#[derive(Debug, Clone, FromRow)]
pub struct ScheduleStats {
    pub active_orders: i64,      // 活跃工单数（status IN 2,3）
    pub pending_batches: i64,    // 待排产（status=1）
    pub in_progress_batches: i64, // 进行中（status IN 2,3）
    pub pending_receipt_batches: i64, // 待入库（status=4）
    pub completed_batches: i64,  // 已完成（status=5）
}

/// 看板卡片
#[derive(Debug, Clone, FromRow)]
pub struct ScheduleCard {
    pub id: i64,
    pub batch_no: String,
    pub product_name: Option<String>,
    pub batch_qty: Decimal,
    pub completed_qty: Decimal,
    pub current_step: i32,
    pub total_steps: Option<i32>,
    pub current_step_name: Option<String>,
    pub status: BatchStatus,  // 需要引用 mes::enums
    pub work_order_id: i64,
    pub wo_doc_number: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

**`abt-core/src/mes/dashboard/repo.rs`** — 新增方法：

```rust
pub async fn get_schedule_stats(executor: &mut PgConnection) -> Result<ScheduleStats>
pub async fn get_schedule_cards(executor: &mut PgConnection, status: Option<BatchStatus>) -> Result<Vec<ScheduleCard>>
```

SQL 核心逻辑：
- `get_schedule_stats`: 5 个 COUNT 子查询
- `get_schedule_cards`: JOIN production_batches + work_orders + products + work_order_routings，按 status 分组

**`abt-core/src/mes/dashboard/service.rs`** — trait 新增：

```rust
async fn get_schedule_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<ScheduleStats>;
async fn get_schedule_cards(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<Vec<ScheduleCard>>;
```

**`abt-core/src/mes/dashboard/implt.rs`** — 实现两个方法。

### Step 2: 前端 — 重写 `mes_schedule_board.rs`

页面布局：

1. **统计行** — 5 个 `stat-card`，使用已有 CSS 类
   - 活跃工单 / 待排产 / 进行中 / 待入库 / 已完成

2. **看板区** — 4 列 CSS Grid 布局
   - 每列标题 + 计数标签
   - 列内卡片列表
   - 卡片内容：
     - 批次号（`mono`）
     - 产品名
     - 数量：已完成/计划
     - 进度条（CSS width 百分比）
     - 标签：工单号

3. **进度条** — 纯 CSS 实现：
   ```html
   <div class="progress-bar"><div class="progress-fill" style="width: 60%"></div></div>
   ```
   需在 `uno.config.ts` 添加 `progress-bar` 和 `progress-fill` shortcuts

### Step 3: CSS — 添加看板样式

**`uno.config.ts`** — 在 preflight 中新增：

```css
.kanban-board { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--space-4); }
.kanban-column { ... min-height: 400px; }
.kanban-column-header { ... }
.kanban-card { ... cursor: pointer; }
.kanban-card:hover { ... }
.progress-bar { height: 6px; background: rgba(0,0,0,0.06); border-radius: 3px; }
.progress-fill { height: 100%; border-radius: 3px; transition: width 0.3s; }
```

### 涉及文件

| 文件 | 改动 |
|------|------|
| `abt-core/src/mes/dashboard/model.rs` | 新增 `ScheduleStats` + `ScheduleCard` |
| `abt-core/src/mes/dashboard/repo.rs` | 新增 2 个查询方法 |
| `abt-core/src/mes/dashboard/service.rs` | trait 新增 2 方法 |
| `abt-core/src/mes/dashboard/implt.rs` | 实现 2 方法 |
| `abt-web/src/pages/mes_schedule_board.rs` | 完整重写 |
| `uno.config.ts` | 新增看板 CSS 类 |
