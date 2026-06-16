# 排程看板重构方案

> 对标 Odoo `mrp_workorder` Calendar + Work Center Load 视图
> 前置条件：migration 051（work_centers / work_calendars / work_center_bookings）已执行

## 一、问题诊断

### 现状：排程看板不展示排程结果

| 组件 | 状态 | 位置 |
|---|---|---|
| `work_center` / `work_calendar` 模块 | ✅ 已实现 | `master_data/work_center/`、`work_calendar/` |
| `find_available_slot` / `create_booking` | ✅ 已实现 | `work_calendar/service.rs` |
| `schedule()` 工序级正向排程 | ✅ 已实现 | `production_plan/implt.rs:408-551` |
| **排程看板页面** | ❌ **不读 booking，只读批次状态** | `mes_schedule_board.rs` |

看板两个视图都不展示排程结果：
- "状态看板"：按 `BatchStatus` 分组卡片，数据源是 `production_batches`
- "工作中心排程"：只是 `work_centers` 列表表格，无时间轴、无负荷、无占用

排程算法产出的 `work_center_bookings` 没有任何页面消费。

### Odoo 对标

Odoo `mrp_workorder_views.xml` 三视图组合：
1. **Calendar View**（核心）：横轴时间，`color="workcenter_id"` 按工作中心着色，色块跨 `date_start ~ date_finished`
2. **Work Center Load Pivot/Graph**：工作中心 × 日期 的 `duration_expected` 矩阵
3. **Kanban**：按状态分组，带 `date_start` 和 play/pause/stop 实时状态

---

## 二、接口设计

### 2.1 WorkCalendarService 新增方法

```rust
// work_calendar/service.rs 新增
async fn list_bookings_multi(
    &self,
    db: PgExecutor<'_>,
    work_center_ids: &[i64],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<WorkCenterBooking>>;
```

**Repo 层**（`BookingRepo::list_range_multi`）：
```sql
SELECT id, work_center_id, work_order_id, plan_item_id,
       date_from, date_to, duration_minutes, created_at
FROM work_center_bookings
WHERE work_center_id = ANY($1) AND date_from < $2 AND date_to > $3
ORDER BY work_center_id, date_from
```

### 2.2 MesDashboardService 新增方法

```rust
// dashboard/service.rs 新增
/// 甘特图数据：工作中心列表 + 时间范围内的 booking（含工单/产品/工序信息）
async fn get_gantt_data(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
    work_center_ids: Option<&[i64]>,  // None = 全部活跃工作中心
) -> Result<GanttData>;

/// 负荷分析：工作中心 × 日期 的已排程工时 vs 可用工时
async fn get_work_center_load(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<WcDailyLoad>>;
```

### 2.3 新增 Model（`dashboard/model.rs`）

```rust
/// 甘特图色块（WorkCenterBooking JOIN 工单/产品/工序）
#[derive(Debug, Clone, FromRow)]
pub struct GanttBooking {
    pub booking_id: i64,
    pub work_center_id: i64,
    pub date_from: DateTime<Utc>,
    pub date_to: DateTime<Utc>,
    pub duration_minutes: Decimal,
    pub work_order_id: i64,
    pub wo_doc_number: Option<String>,
    pub plan_item_id: Option<i64>,
    pub product_name: Option<String>,
    pub batch_no: Option<String>,
    pub process_name: Option<String>,
    pub step_order: Option<i32>,
    pub batch_status: Option<i16>,   // production_batches.status
}

/// 甘特图完整数据
#[derive(Debug, Clone)]
pub struct GanttData {
    pub work_centers: Vec<WorkCenterInfo>,  // 行
    pub bookings: Vec<GanttBooking>,         // 色块
    pub date_range: Vec<NaiveDate>,          // 列（日期序列）
}

/// 工作中心简要信息（甘特图行头）
#[derive(Debug, Clone, FromRow)]
pub struct WorkCenterInfo {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub work_center_type: i16,
}

/// 工作中心每日负荷
#[derive(Debug, Clone, FromRow)]
pub struct WcDailyLoad {
    pub work_center_id: i64,
    pub work_center_code: String,
    pub work_center_name: String,
    pub date: NaiveDate,
    pub booked_minutes: Decimal,
    pub available_minutes: Decimal,   // 从日历算出的当日可用工时
    pub load_pct: Decimal,            // booked / available * 100
}
```

### 2.4 Repo SQL（`dashboard/repo.rs`）

**get_gantt_bookings**（一次 JOIN 查询）：
```sql
SELECT b.id AS booking_id, b.work_center_id,
       b.date_from, b.date_to, b.duration_minutes,
       b.work_order_id, wo.doc_number AS wo_doc_number,
       b.plan_item_id,
       p.pdt_name AS product_name,
       pb.batch_no,
       wor.process_name, wor.step_order,
       pb.status AS batch_status
FROM work_center_bookings b
JOIN work_orders wo ON wo.id = b.work_order_id
LEFT JOIN production_batches pb ON pb.work_order_id = b.work_order_id
LEFT JOIN products p ON p.product_id = pb.product_id
LEFT JOIN work_order_routings wor
       ON wor.work_order_id = b.work_order_id
      AND wor.work_center_id = b.work_center_id
WHERE b.work_center_id = ANY($1)
  AND b.date_from < $2 AND b.date_to > $3
ORDER BY b.work_center_id, b.date_from
```

**get_work_center_load**（聚合 + 日历可用工时）：
```sql
-- 步骤1: 按 wc × day 聚合已排程工时
SELECT work_center_id,
       DATE(date_from) AS date,
       SUM(duration_minutes) AS booked_minutes
FROM work_center_bookings
WHERE work_center_id = ANY($1)
  AND date_from >= $2 AND date_from < $3
GROUP BY work_center_id, DATE(date_from)

-- 步骤2: 可用工时从 work_calendar_lines 算（当日 weekday 的工作时段总长）
-- 在 Rust 里按 weekday 查 work_calendar_lines 并乘以天数
```

---

## 三、UI 设计（Odoo 风格）

### 3.1 整体布局

```
┌─────────────────────────────────────────────────────────┐
│  排程看板                                                │
│  [统计卡片行: 活跃工单/待排产/进行中/待入库/已完成]      │
│                                                         │
│  [甘特图] [负荷分析] [状态看板]    日期: [< 本周 >] ▾   │
│  ─────────────────────                                 │
│  (视图内容区)                                           │
└─────────────────────────────────────────────────────────┘
```

- 默认显示**甘特图**视图
- 日期范围：本周起未来 14 天，带前/后翻周按钮 + 日期选择
- 三个 tab 通过 Hyperscript `_=` 切换（纯前端），HTMX 重渲染带日期参数

### 3.2 甘特图视图（核心，对标 Odoo Calendar）

**布局**：CSS Grid，左侧工作中心列固定 160px，右侧每天一列等宽。

```
┌──────────────┬────────┬────────┬────────┬────────┬────────┐
│              │ 06-16  │ 06-17  │ 06-18  │ 06-19  │ 06-20  │
│              │ 周一   │ 周二   │ 周三   │ 周四   │ 周五   │
├──────────────┼────────┼────────┼────────┼────────┼────────┤
│ ● WC-001     │ ██████ │ ██     │        │ ████   │        │
│   注塑车间   │ A-3    │ B-1    │        │ C-2    │        │
├──────────────┼────────┼────────┼────────┼────────┼────────┤
│ ● WC-002     │        │ ██████ │ ██████ │        │ ██     │
│   组装车间   │        │ A-3    │ C-2    │        │ D-1    │
└──────────────┴────────┴────────┴────────┴────────┴────────┘
```

**色块设计（Odoo 风格）**：
- 圆角 `border-radius: 6px`
- 按工单着色（同工单同色）：使用预定义 8 色调色板循环（蓝/绿/紫/橙/青/粉/靛/琥珀）
- 左侧 3px 实色边条标识状态：蓝=待开始、绿=进行中、灰=已完成、红=异常
- 内容：`工单号 · 工序名`，字号 `text-xs`，白色文字
- `box-shadow: 0 1px 3px rgba(0,0,0,0.12)` 轻阴影
- `transition: transform 0.15s, box-shadow 0.15s`，hover 时 `transform: translateY(-1px)` + 加深阴影
- 今天所在列加 `background: var(--today-bg)` 淡蓝底色 + 顶部 2px 蓝色边条
- 周末列加 `opacity: 0.6` 灰底色

**行头设计**：
- 左侧工作中心名称前加 8px 圆点，颜色对应该工作中心的标识色
- 名称下方 `text-xs text-gray` 显示类型（机器/人工/委外）

**跨天色块处理**：
- 用 `grid-column: start / span N` 实现跨天
- Rust 端计算每个 booking 跨越的列数（`date_to.date - date_from.date + 1`）

### 3.3 负荷分析视图（对标 Odoo Work Center Load）

**布局**：工作中心 × 日期矩阵，单元格为热力色块。

```
┌──────────────┬────────┬────────┬────────┬────────┬────────┐
│              │ 06-16  │ 06-17  │ 06-18  │ 06-19  │ 06-20  │
├──────────────┼────────┼────────┼────────┼────────┼────────┤
│ WC-001 注塑  │  85%   │ 100%   │  40%   │   0%   │  60%   │
│              │  🟡    │  🔴    │  🟢    │  ⚪    │  🟢    │
├──────────────┼────────┼────────┼────────┼────────┼────────┤
│ WC-002 组装  │  50%   │  70%   │  90%   │ 100%   │  30%   │
│              │  🟢    │  🟢    │  🟡    │  🔴    │  🟢    │
└──────────────┴────────┴────────┴────────┴────────┴────────┘
```

**热力色块**：
- 每个单元格是一个圆角方块 `border-radius: 8px`，尺寸填满格
- 负荷率映射背景色：
  - 0% → `var(--color-gray-100)` 灰白 + 文字灰
  - 1-70% → `var(--color-green-*)` 绿色渐变（越深负荷越高）
  - 71-90% → `var(--color-amber-*)` 琥珀色
  - 91-100%+ → `var(--color-red-*)` 红色
- 负荷率百分比居中显示，白色或深色文字（根据背景亮度）
- hover 时 `title` 属性显示 `已排 Xh / 可用 Yh`

### 3.4 状态看板视图（保留优化）

现有逻辑保留，补充：
- 卡片增加排程起始日期（booking 最早 `date_from`）角标
- 进行中卡片增加当前工序的工作中心名称
- 卡片视觉微调：圆角加大、增加 hover 阴影

### 3.5 统计卡片行视觉

对标 Odoo dashboard 的 KPI 卡片：
- 每张卡片 `border-radius: 12px`、`box-shadow: 0 1px 4px rgba(0,0,0,0.06)`
- 左侧 4px 彩色边条（对应统计项语义色）
- 大号数字 + 小号标签，数字用 `font-weight: 700`

---

## 四、实现步骤

### 后端（abt-core）

1. **`work_calendar/repo.rs`**：`BookingRepo` 加 `list_range_multi(ids, from, to)`
2. **`work_calendar/service.rs` + `implt.rs`**：加 `list_bookings_multi` trait 方法 + 实现
3. **`dashboard/model.rs`**：加 `GanttBooking` / `GanttData` / `WorkCenterInfo` / `WcDailyLoad`
4. **`dashboard/repo.rs`**：加 `get_gantt_bookings` + `get_work_center_load` + `get_work_center_load_with_calendar`（聚合 + 日历可用工时计算）
5. **`dashboard/service.rs` + `implt.rs`**：加 `get_gantt_data` + `get_work_center_load` trait 方法 + 实现

### 前端（abt-web）

6. **`mes_schedule_board.rs`** 重写：
   - `get_schedule_board` handler 支持 query params（`from`/`to`/`view`）用于 HTMX 重渲染
   - `gantt_view()` — 甘特图渲染
   - `load_view()` — 负荷分析渲染
   - `kanban_view()` — 状态看板（保留现有）
7. **`base.css`**：新增 `.gantt-*`、`.load-matrix-*` 样式（Odoo 风格）
8. **`routes/mes_batch.rs`**：`ScheduleBoardPath` 支持 query 参数

### 验证

9. `cargo clippy -p abt-core && cargo clippy -p abt-web`
10. 浏览器访问 `/admin/mes/schedule-board` 验证三视图

---

## 五、验收标准

- [ ] 甘特图展示工作中心 × 日期矩阵，booking 色块跨天正确
- [ ] 甘特图色块按工单着色，hover 有效果，今天列高亮
- [ ] 负荷分析展示工作中心 × 日期热力矩阵，颜色按负荷率分级
- [ ] 负荷率 = 已排工时 / 日历可用工时，计算正确
- [ ] 状态看板保留，卡片含排程起始日期
- [ ] 日期范围切换（前/后翻周）通过 HTMX 重渲染
- [ ] 无 booking 数据时显示空状态提示
- [ ] `cargo clippy` 无错误
