# UI-06: 排程看板增强

> 核心改动牵涉：排程算法重写 (schedule_v2) — 前向排程、工作中心产能、日历可用时段
> Odoo 参考：`mrp.workcenter` Gantt 视图

## 1. 目标

排程看板从纯"按状态分列的 Kanban"升级为"双视图"系统：保留 Kanban 状态视图，新增按工作中心×时间轴的排程视图，对接 schedule_v2 的排程结果。

## 2. 当前状态

### 已有

`mes_schedule_board.rs` — Kanban 四列看板：
- 统计行：活跃工单 / 待排产 / 进行中 / 待入库 / 已完成
- Kanban 列：待排产 / 进行中 / 待入库 / 已完成
- 卡片：批次编号 / 产品 / 完工进度 / 当前工序 / 工单编号

### 缺失

| 差距 | 说明 |
|------|------|
| 无工作中心维度 | 无法看到每个工作中心的负载/排程 |
| 无时间轴 | 无计划开始/结束时间的可视化 |
| 无产能利用率 | 看不到工作中心的繁忙/空闲程度 |
| 卡片缺排程信息 | 无 scheduled_start / scheduled_end |
| 无排程触发入口 | 无法从看板直接触发重新排程 |

## 3. Odoo 参考

### Odoo Work Center Gantt

```
        │ 06/15 周六 │ 06/16 周日 │ 06/17 周一 │ 06/18 周二 │
────────┼────────────┼────────────┼────────────┼────────────┤
WC001   │ ████████   │ ████████   │            │            │
注塑机A  │ MO-001 注塑 │ MO-003 注塑 │  (空闲)    │  (空闲)    │
        │ 08:00-16:00│ 08:00-16:00│            │            │
────────┼────────────┼────────────┼────────────┼────────────┤
WC002   │            │ ████████   │ ████████   │            │
组装线B  │  (空闲)    │ MO-001 组装 │ MO-002 组装 │  (空闲)    │
        │            │ 08:00-12:00│ 08:00-16:00│            │
────────┼────────────┼────────────┼────────────┼────────────┤
WC003   │            │            │            │ ████████   │
检测台C  │  (空闲)    │  (空闲)    │  (空闲)    │ MO-001 检测 │
        │            │            │            │ 08:00-10:00│
```

**关键 Odoo 模式**：
1. **行 = 工作中心，列 = 日期** — 矩阵视图
2. **颜色编码** — 按工单或状态着色
3. **产能条** — 每个工作中心显示日产能利用百分比
4. **点击拖拽** — 重新分配（我们不实现拖拽）

### 我们的适配

SSR + HTMX 架构，不做拖拽。采用 HTML 表格矩阵（工作中心行 × 日期列），每个单元格渲染排程块。通过 HTMX 切换视图模式和日期范围。

## 4. 双视图设计

### 4.1 视图切换

```
┌─────────────────────────────────────────────────────────────┐
│ 排程看板                                                     │
│                                                              │
│ [状态看板] [工作中心排程]     ← Hyperscript 切换 Tab          │
│                                                              │
│ ─── 当前视图 ───                                             │
└─────────────────────────────────────────────────────────────┘
```

**实现**（Hyperscript 纯前端切换，不涉及服务端）：

```rust
fn view_toggle(active: &str) -> Markup {
    html! {
        div class="view-toggle" {
            button class=(if active == "kanban" { "tab-btn active" } else { "tab-btn" })
                _="on click remove .active from .tab-btn then add .active to me then add .hidden to #gantt-view then remove .hidden from #kanban-view" {
                "状态看板"
            }
            button class=(if active == "gantt" { "tab-btn active" } else { "tab-btn" })
                _="on click remove .active from .tab-btn then add .active to me then add .hidden to #kanban-view then remove .hidden from #gantt-view" {
                "工作中心排程"
            }
        }
    }
}
```

### 4.2 视图 A — 状态看板（增强现有 Kanban）

保留现有 4 列 Kanban，增强卡片内容：

**增强后卡片**：

```
┌──────────────────────────────────────┐
│ B001              ● 进行中           │
│ 电源板A                             │
│ 95 / 100   ████████████████░ 95%   │
│ 工序: 2/3 组装                      │
│ 工单: MO-2024-001                   │
│ ── 新增 ──                          │
│ 工作中心: WC002 组装线B             │
│ 计划: 06/15 08:00 → 06/15 12:00     │
└──────────────────────────────────────┘
```

**新增字段**：
- `work_center_name` — 当前工作中心
- `scheduled_start` / `scheduled_end` — 计划时间
- `planned_duration` — 计划工时

**ScheduleCard 模型增强**（`abt-core/src/mes/dashboard/model.rs`）：

```rust
pub struct ScheduleCard {
    // 现有字段...
    pub work_center_id: Option<i64>,
    pub work_center_name: Option<String>,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub planned_duration_hours: Option<Decimal>,
}
```

### 4.3 视图 B — 工作中心排程（新）

```
┌─────────────────────────────────────────────────────────────┐
│ 排程范围: 2024-06-15 ~ 2024-06-21    [◀ 上一周] [下一周 ▶]  │
│ 日历: 标准工作日                    [重新排程]               │
├─────────────────────────────────────────────────────────────┤
│              │ 06/15    │ 06/16    │ 06/17    │ 06/18    │
│              │ 周六     │ 周日     │ 周一     │ 周二     │
├──────────────┼──────────┼──────────┼──────────┼──────────┤
│ WC001        │ ████████ │ ████████ │          │          │
│ 注塑机A      │ MO-001   │ MO-003   │  空闲    │  空闲    │
│ 利用率       │ 80%      │ 80%      │  0%      │  0%      │
├──────────────┼──────────┼──────────┼──────────┼──────────┤
│ WC002        │          │ ████     │ ████████ │          │
│ 组装线B      │  空闲    │ MO-001   │ MO-002   │  空闲    │
│ 利用率       │  0%      │ 50%      │ 80%      │  0%      │
├──────────────┼──────────┼──────────┼──────────┼──────────┤
│ WC003        │          │          │          │ ████     │
│ 检测台C      │  空闲    │  空闲    │  空闲    │ MO-001   │
│ 利用率       │  0%      │  0%      │  0%      │ 25%      │
└──────────────┴──────────┴──────────┴──────────┴──────────┘
```

**排程块**（单元格内）：

```
┌────────────────────────┐
│ MO-001 注塑  (color)   │
│ 08:00 - 16:00          │
│ B001 · 100件           │
└────────────────────────┘
```

颜色编码：
- 蓝色块 — Released 工单
- 绿色块 — InProduction 工单
- 橙色块 — Suspended 工单
- 红色块 — 超期（scheduled_end < now 且未完工）

### 4.4 实现

```rust
fn gantt_view(
    work_centers: &[GanttWorkCenter],
    date_range: &[NaiveDate],
    schedule_items: &[GanttScheduleItem],
) -> Markup {
    html! {
        div class="gantt-container" id="gantt-view" {
            // 日期范围控制
            div class="gantt-controls" {
                span class="gantt-range-label" {
                    (date_range.first().map(|d| d.format("%Y-%m-%d")).unwrap_or_default())
                    " ~ "
                    (date_range.last().map(|d| d.format("%Y-%m-%d")).unwrap_or_default())
                }
                // 上一周 / 下一周 — HTMX GET 带日期参数
                button class="btn btn-sm btn-default"
                    hx-get=(ScheduleBoardPath::PATH)
                    hx-vals=(format!(r#"{{"view":"gantt","start_date":"{}"}}"#, prev_week))
                    hx-target="#gantt-view"
                    hx-swap="outerHTML" {
                    "◀"
                }
                button class="btn btn-sm btn-default"
                    hx-get=(ScheduleBoardPath::PATH)
                    hx-vals=(format!(r#"{{"view":"gantt","start_date":"{}"}}"#, next_week))
                    hx-target="#gantt-view"
                    hx-swap="outerHTML" {
                    "▶"
                }
            }
            // Gantt 表格
            div class="gantt-table-wrap" {
                table class="gantt-table" {
                    thead {
                        tr {
                            th class="gantt-row-header" { "工作中心" }
                            @for date in date_range {
                                th class="gantt-col-header" {
                                    div { (date.format("%m/%d").to_string()) }
                                    div class="gantt-day-name" { (weekday_cn(date.weekday())) }
                                }
                            }
                        }
                    }
                    tbody {
                        @for wc in work_centers {
                            tr {
                                td class="gantt-row-header" {
                                    div class="wc-name" { (wc.name) }
                                    div class="wc-code mono muted" { (wc.code) }
                                }
                                @for date in date_range {
                                    td class="gantt-cell" {
                                        (render_gantt_cell(wc.id, *date, schedule_items))
                                    }
                                }
                            }
                            // 利用率行
                            tr class="gantt-util-row" {
                                td class="gantt-row-header" {
                                    span class="muted small" { "利用率" }
                                }
                                @for date in date_range {
                                    td class="gantt-util-cell" {
                                        (utilization_badge(wc.id, *date, schedule_items, wc.daily_capacity))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_gantt_cell(wc_id: i64, date: NaiveDate, items: &[GanttScheduleItem]) -> Markup {
    let day_items: Vec<_> = items.iter()
        .filter(|i| i.work_center_id == wc_id && i.scheduled_date == date)
        .collect();
    if day_items.is_empty() {
        html! { span class="gantt-empty-cell" { "—" } }
    } else {
        html! {
            div class="gantt-blocks" {
                @for item in day_items {
                    a class=(format!("gantt-block status-{}", item.status_class()))
                       href=(format!("/admin/mes/orders/{}", item.work_order_id))
                       title=(format!("{} {} → {}", item.wo_doc_number, item.time_range(), item.product_name)) {
                        div class="gantt-block-title" {
                            (item.wo_doc_number) " " (item.process_name)
                        }
                        div class="gantt-block-meta mono" {
                            (item.time_range())
                        }
                        div class="gantt-block-batch muted" {
                            (item.batch_no) " · " (fmt_qty(item.qty)) "件"
                        }
                    }
                }
            }
        }
    }
}
```

### 4.5 重新排程入口

```
┌── 重新排程 ──────────────────────────────┐
│                                  [×]    │
│ 选择排程范围：                           │
│   ○ 所有待排产工单                      │
│   ○ 指定日期范围                        │
│        开始: [2024-06-15]               │
│        结束: [2024-06-30]               │
│                                         │
│ 排程策略：                               │
│   ○ 前向排程（从最早可用开始）           │
│   ○ 后向排程（从交期倒推）               │
│                                         │
│                         [取消] [开始排程] │
└─────────────────────────────────────────┘
```

**实现**：

```rust
fn reschedule_modal() -> Markup {
    html! {
        button class="btn btn-primary"
            _="on click add .is-open to #reschedule-modal" {
            "重新排程"
        }
        div class="modal-overlay" id="reschedule-modal" {
            div class="modal" {
                div class="modal-head" {
                    h3 { "重新排程" }
                    button _="on click remove .is-open from #reschedule-modal" { "×" }
                }
                form class="modal-body"
                    hx-post="/admin/mes/schedule/run"
                    hx-redirect="/admin/mes/schedule?view=gantt" {
                    div class="form-field" {
                        label { "排程范围" }
                        div class="radio-group" {
                            label { input type="radio" name="scope" value="pending" checked {}; " 所有待排产工单" }
                            label { input type="radio" name="scope" value="date_range" {}; " 指定日期范围" }
                        }
                    }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "开始日期" }
                            input type="date" class="form-input" name="start_date";
                        }
                        div class="form-field" {
                            label { "结束日期" }
                            input type="date" class="form-input" name="end_date";
                        }
                    }
                    div class="form-field" {
                        label { "排程策略" }
                        div class="radio-group" {
                            label { input type="radio" name="strategy" value="forward" checked {}; " 前向排程" }
                            label { input type="radio" name="strategy" value="backward" {}; " 后向排程" }
                        }
                    }
                }
                div class="modal-foot" {
                    button class="btn btn-default"
                        _="on click remove .is-open from #reschedule-modal" {
                        "取消"
                    }
                    button class="btn btn-primary" type="submit"
                        hx-confirm="重新排程将覆盖现有排程结果，确认？" {
                        "开始排程"
                    }
                }
            }
        }
    }
}
```

**新路由**：

```rust
#[derive(TypedPath, Deserialize)]
#[typed_path("/admin/mes/schedule/run")]
pub struct ScheduleRunPath;

#[require_permission("WORK_ORDER", "update")]
pub async fn post_schedule_run(
    _path: ScheduleRunPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ScheduleRunForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state.pool.begin().await?;
    state.production_plan_service()
        .schedule_v2(&service_ctx, &mut tx, form.into_req()).await?;
    tx.commit().await?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", "/admin/mes/schedule?view=gantt")
        .body(axum::body::Body::empty()).unwrap())
}
```

## 5. 统计行增强

现有 5 个 stat-card，新增：

```
活跃工单: 12    待排产: 5    进行中: 4    待入库: 2    已完成: 8
排程利用率: 68%    超期工单: 1    本周交付: 3
```

```rust
// 新增 stat cards
div class="stat-card" {
    div class="stat-card-value" style="color:var(--primary)" { (utilization_pct) "%" }
    div class="stat-card-label" { "排程利用率" }
}
div class="stat-card" {
    div class="stat-card-value stat-danger" { (overdue_count) }
    div class="stat-card-label" { "超期工单" }
}
```

## 6. 数据模型新增

### `abt-core/src/mes/dashboard/model.rs`

```rust
/// Gantt 视图数据模型
pub struct GanttViewData {
    pub work_centers: Vec<GanttWorkCenter>,
    pub date_range: Vec<chrono::NaiveDate>,
    pub items: Vec<GanttScheduleItem>,
}

pub struct GanttWorkCenter {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub daily_capacity: Decimal,
}

pub struct GanttScheduleItem {
    pub work_order_id: i64,
    pub wo_doc_number: String,
    pub batch_id: Option<i64>,
    pub batch_no: Option<String>,
    pub work_center_id: i64,
    pub process_name: String,
    pub product_name: String,
    pub scheduled_date: chrono::NaiveDate,
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
    pub qty: Decimal,
    pub status: WorkOrderStatus,
}

impl GanttScheduleItem {
    pub fn status_class(&self) -> &'static str {
        match self.status {
            WorkOrderStatus::Released => "released",
            WorkOrderStatus::InProduction => "in-production",
            _ => "other",
        }
    }
    pub fn time_range(&self) -> String {
        format!("{} - {}", self.start_time.format("%H:%M"), self.end_time.format("%H:%M"))
    }
}
```

### DashboardService 新增方法

```rust
async fn get_gantt_view(
    &self, ctx: &ServiceContext, db: impl PgExecutor<'_>,
    start_date: NaiveDate, end_date: NaiveDate,
) -> Result<GanttViewData>;
```

## 7. CSS 新增

```css
/* 视图切换 */
.view-toggle {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border);
    margin-bottom: 16px;
}
.tab-btn {
    padding: 8px 16px;
    border: none;
    background: none;
    cursor: pointer;
    color: var(--muted);
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
}
.tab-btn.active {
    color: var(--primary);
    border-bottom-color: var(--primary);
    font-weight: 500;
}
.tab-btn:hover { color: var(--text); }

/* Gantt 表格 */
.gantt-table-wrap {
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
}
.gantt-table {
    width: 100%;
    border-collapse: collapse;
    min-width: 800px;
}
.gantt-table th,
.gantt-table td {
    border: 1px solid var(--border);
    padding: 0;
    vertical-align: top;
}
.gantt-row-header {
    width: 140px;
    min-width: 140px;
    padding: 8px 12px !important;
    background: var(--gray-50);
    font-weight: 500;
}
.gantt-col-header {
    text-align: center;
    padding: 8px !important;
    background: var(--gray-50);
    font-size: var(--text-sm);
}
.gantt-day-name {
    color: var(--muted);
    font-size: var(--text-xs);
}
.gantt-cell {
    min-width: 120px;
    height: 80px;
    padding: 4px !important;
}
.gantt-empty-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--gray-300);
}

/* Gantt 排程块 */
.gantt-block {
    display: block;
    padding: 4px 8px;
    margin-bottom: 4px;
    border-radius: var(--radius-sm);
    text-decoration: none;
    color: var(--text);
    font-size: var(--text-xs);
    transition: transform 0.15s;
}
.gantt-block:hover { transform: translateY(-1px); box-shadow: var(--shadow-sm); }
.gantt-block.status-released { background: rgba(0, 122, 255, 0.08); border-left: 3px solid var(--info); }
.gantt-block.status-in-production { background: rgba(82, 196, 26, 0.08); border-left: 3px solid var(--success); }
.gantt-block.status-suspended { background: rgba(255, 159, 67, 0.08); border-left: 3px solid var(--warning); }
.gantt-block-title { font-weight: 500; }
.gantt-block-meta { font-size: 10px; color: var(--muted); }
.gantt-block-batch { font-size: 10px; }

/* 利用率行 */
.gantt-util-row { background: var(--gray-25); }
.gantt-util-cell {
    text-align: center;
    padding: 4px !important;
    font-size: var(--text-xs);
}

/* 统计卡片增强 */
.stat-card-value.stat-danger { color: var(--danger); }
```

## 8. 实现步骤

1. `abt-core/src/mes/dashboard/model.rs`:
   - `ScheduleCard` 加 4 个字段（work_center_id, work_center_name, scheduled_start/end, planned_duration）
   - 新增 `GanttViewData` / `GanttWorkCenter` / `GanttScheduleItem` 模型

2. `abt-core/src/mes/dashboard/service.rs` + `implt.rs`:
   - `get_schedule_cards` 查询加 JOIN work_center + work_order.scheduled_start/end
   - 新增 `get_gantt_view` 方法

3. `mes_schedule_board.rs`:
   - 新增 `view_toggle` — Kanban/Gantt 切换
   - 新增 `gantt_view` — 矩阵视图
   - 新增 `reschedule_modal` — 重新排程弹窗
   - 增强 `kanban_card` — 加工作中心 + 计划时间

4. `routes/mes_batch.rs`:
   - ScheduleBoardPath 加 `view` / `start_date` 参数
   - 新增 `ScheduleRunPath` + `post_schedule_run` handler

5. `base.css` — 加 gantt / view-toggle 样式

## 9. 验收标准

- [ ] 视图切换在 Kanban 和 Gantt 之间无缝切换（Hyperscript 纯前端）
- [ ] Gantt 视图按工作中心行 × 日期列渲染排程块
- [ ] 排程块颜色按工单状态区分
- [ ] 排程块点击跳转工单详情
- [ ] 利用率行正确计算每个工作中心每天的利用率
- [ ] 重新排程弹窗可触发 schedule_v2
- [ ] Kanban 卡片显示工作中心 + 计划时间
- [ ] 统计行显示排程利用率和超期工单数
- [ ] cargo clippy 零错误
