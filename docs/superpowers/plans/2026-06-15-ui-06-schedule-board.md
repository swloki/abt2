# 排程看板增强 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 排程看板从单 Kanban 视图升级为双视图（Kanban + 工作中心 Gantt 矩阵），增强卡片内容，新增重新排程入口。

**Architecture:** 修改 `mes_schedule_board.rs` 新增 view_toggle 和 gantt_view 函数。在 abt-core dashboard 模块中扩展 ScheduleCard 和新增 GanttViewData 模型。

**Tech Stack:** Rust (Maud + HTMX + Hyperscript), abt-core MesDashboardService / WorkCenterService

---

## File Structure

| 文件 | 职责 | 动作 |
|------|------|------|
| `abt-core/src/mes/dashboard/model.rs` | ScheduleCard 加字段 + 新增 GanttViewData 模型 | Modify |
| `abt-core/src/mes/dashboard/implt.rs` | get_schedule_cards 查询增强 + 新增 get_gantt_view | Modify |
| `abt-core/src/mes/dashboard/service.rs` | 新增 get_gantt_view trait 方法 | Modify |
| `abt-web/src/pages/mes_schedule_board.rs` | view_toggle + gantt_view + reschedule_modal + 增强 kanban_card | Modify |
| `abt-web/src/routes/mes_batch.rs` | ScheduleBoardPath 加 view 参数 + ScheduleRunPath | Modify |
| `static/base.css` | gantt / view-toggle 样式 | Modify |

---

## Task 1: abt-core — ScheduleCard 模型扩展

**Files:**
- Modify: `abt-core/src/mes/dashboard/model.rs`

- [ ] **Step 1: 在 ScheduleCard 结构体中添加字段**

找到 `ScheduleCard` struct 定义，添加：

```rust
    pub work_center_id: Option<i64>,
    pub work_center_name: Option<String>,
    pub scheduled_start: Option<chrono::DateTime<chrono::Utc>>,
    pub scheduled_end: Option<chrono::DateTime<chrono::Utc>>,
```

注意：这些字段需要 `#[sqlx(default)]` 以兼容现有数据，或者直接修改 SQL 查询添加 JOIN。

- [ ] **Step 2: 新增 Gantt 模型**

在 model.rs 末尾添加：

```rust
// ── Gantt 视图模型 ──

#[derive(Debug, Clone)]
pub struct GanttViewData {
    pub work_centers: Vec<GanttWorkCenter>,
    pub date_range: Vec<chrono::NaiveDate>,
    pub items: Vec<GanttScheduleItem>,
}

#[derive(Debug, Clone)]
pub struct GanttWorkCenter {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub daily_capacity: rust_decimal::Decimal,
}

#[derive(Debug, Clone)]
pub struct GanttScheduleItem {
    pub work_order_id: i64,
    pub wo_doc_number: String,
    pub batch_id: Option<i64>,
    pub batch_no: Option<String>,
    pub work_center_id: i64,
    pub process_name: String,
    pub product_name: String,
    pub scheduled_date: chrono::NaiveDate,
    pub qty: rust_decimal::Decimal,
    pub status: crate::mes::enums::WorkOrderStatus,
}

impl GanttScheduleItem {
    pub fn status_class(&self) -> &'static str {
        use crate::mes::enums::WorkOrderStatus;
        match self.status {
            WorkOrderStatus::Released => "released",
            WorkOrderStatus::InProduction => "in-production",
            _ => "other",
        }
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | grep "^error"`

- [ ] **Step 4: Commit**

```bash
git add abt-core/src/mes/dashboard/model.rs
git commit -m "feat: extend ScheduleCard and add Gantt models"
```

---

## Task 2: abt-core — Dashboard Service 扩展

**Files:**
- Modify: `abt-core/src/mes/dashboard/service.rs`
- Modify: `abt-core/src/mes/dashboard/implt.rs`

- [ ] **Step 1: 在 MesDashboardService trait 中添加方法**

在 service.rs 的 trait 定义中添加：

```rust
    async fn get_gantt_view(
        &self,
        ctx: &ServiceContext,
        db: crate::shared::types::PgExecutor<'_>,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> crate::shared::types::Result<super::model::GanttViewData>;
```

- [ ] **Step 2: 在 implt.rs 中实现 get_gantt_view**

```rust
    async fn get_gantt_view(
        &self,
        ctx: &ServiceContext,
        db: crate::shared::types::PgExecutor<'_>,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> crate::shared::types::Result<super::model::GanttViewData {
        use super::model::*;

        // 1. 查所有活跃工作中心
        let work_centers: Vec<GanttWorkCenter> = sqlx::query_as(
            r#"SELECT id, code, name, default_capacity as daily_capacity
               FROM work_centers WHERE is_active = true ORDER BY code"#
        )
        .fetch_all(&mut *db)
        .await
        .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;

        // 2. 生成日期范围
        let date_range: Vec<chrono::NaiveDate> = {
            let mut dates = Vec::new();
            let mut current = start_date;
            while current <= end_date {
                dates.push(current);
                current += chrono::Duration::days(1);
            }
            dates
        };

        // 3. 查排程占用
        let items: Vec<GanttScheduleItem> = sqlx::query_as(
            r#"SELECT
                wo.id as work_order_id,
                wo.doc_number as wo_doc_number,
                b.id as batch_id,
                b.batch_no,
                wc.id as work_center_id,
                rp.process_name,
                p.name as product_name,
                DATE(wc_booking.date_from) as scheduled_date,
                wo.planned_qty as qty,
                wo.status
               FROM work_center_bookings wc_booking
               JOIN work_orders wo ON wc_booking.work_order_id = wo.id
               LEFT JOIN production_batches b ON wc_booking.work_order_id = b.work_order_id
               LEFT JOIN work_order_routings rp ON rp.work_order_id = wo.id AND rp.work_center_id = wc_booking.work_center_id
               LEFT JOIN products p ON wo.product_id = p.id
               WHERE wc_booking.date_from >= $1 AND wc_booking.date_to <= $2
               AND wo.deleted_at IS NULL"#,
        )
        .bind(start_date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        .bind(end_date.and_hms_opt(23, 59, 59).unwrap().and_utc())
        .fetch_all(&mut *db)
        .await
        .map_err(|e| crate::shared::types::DomainError::Internal(e.into()))?;

        Ok(GanttViewData { work_centers, date_range, items })
    }
```

注意：SQL 查询需要根据实际表结构调整。`work_center_bookings` 表名和字段名需要与 migration 046 一致。

- [ ] **Step 3: 增强 get_schedule_cards 查询**

在 `get_schedule_cards` 的 SQL 查询中添加 JOIN 获取 work_center 信息：

```sql
-- 在现有查询中添加：
LEFT JOIN work_order_routings wor ON wor.work_order_id = wo.id
    AND wor.step_no = b.current_step
LEFT JOIN work_centers wc ON wor.work_center_id = wc.id
-- SELECT 中添加：
wc.id as work_center_id,
wc.name as work_center_name,
wo.scheduled_start,
wo.scheduled_end
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | grep "^error"`

- [ ] **Step 5: Commit**

```bash
git add abt-core/src/mes/dashboard/service.rs abt-core/src/mes/dashboard/implt.rs
git commit -m "feat: add get_gantt_view and enhance get_schedule_cards"
```

---

## Task 3: 视图切换 + Gantt 视图渲染

**Files:**
- Modify: `abt-web/src/pages/mes_schedule_board.rs`

- [ ] **Step 1: 修改 handler 支持 view 参数**

修改 `get_schedule_board` handler，添加 view 参数解析：

```rust
#[require_permission("WORK_ORDER", "read")]
pub async fn get_schedule_board(
    _path: ScheduleBoardPath,
    ctx: RequestContext,
    axum::extract::Query(params): axum::extract::Query<ScheduleBoardParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.mes_dashboard_service();
    let stats = svc.get_schedule_stats(&service_ctx, &mut conn).await?;
    let cards = svc.get_schedule_cards(&service_ctx, &mut conn).await?;

    let view = params.view.as_deref().unwrap_or("kanban");
    let gantt_data = if view == "gantt" {
        let today = chrono::Local::now().date_naive();
        let start = params.start_date
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .unwrap_or(today);
        let end = start + chrono::Duration::days(6);
        Some(svc.get_gantt_view(&service_ctx, &mut conn, start, end).await?)
    } else {
        None
    };

    let content = schedule_board_page(&stats, &cards, view, gantt_data.as_ref());
    Ok(Html(admin_page(
        is_htmx, "排程看板", &claims, "production",
        ScheduleBoardPath::PATH, "生产管理", None, content, &nav_filter,
    ).into_string()))
}

#[derive(Debug, serde::Deserialize, Default)]
pub struct ScheduleBoardParams {
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
}
```

- [ ] **Step 2: 修改 schedule_board_page 添加 view_toggle**

在 `schedule_board_page` 函数中，在统计行后添加视图切换：

```rust
fn schedule_board_page(
    stats: &ScheduleStats,
    cards: &[ScheduleCard],
    view: &str,
    gantt_data: Option<&GanttViewData>,
) -> Markup {
    // ... 统计行（不变）...

    // 视图切换
    div class="view-toggle" {
        button class={ @if view == "kanban" { "tab-btn active" } @else { "tab-btn" } }
            _="on click remove .active from .tab-btn then add .active to me then remove .hidden from #kanban-view then add .hidden to #gantt-view" {
            "状态看板"
        }
        button class={ @if view == "gantt" { "tab-btn active" } @else { "tab-btn" } }
            _="on click remove .active from .tab-btn then add .active to me then remove .hidden from #gantt-view then add .hidden to #kanban-view" {
            "工作中心排程"
        }
    }

    // Kanban 视图
    div id="kanban-view" class={ @if view == "gantt" { "hidden" } @else { "" } } {
        // ... 现有 Kanban 四列 ...
    }

    // Gantt 视图
    div id="gantt-view" class={ @if view == "kanban" { "hidden" } @else { "" } } {
        @if let Some(data) = gantt_data {
            (gantt_view(data))
        } @else {
            div class="empty-row" { "切换到工作中心排程视图加载..." }
        }
    }
}
```

- [ ] **Step 3: 新增 gantt_view 函数**

```rust
fn gantt_view(data: &GanttViewData) -> Markup {
    use abt_core::mes::dashboard::model::*;

    html! {
        div class="gantt-container" {
            // 重新排程按钮
            div class="gantt-controls" {
                span class="gantt-range-label" {
                    (data.date_range.first().map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default())
                    " ~ "
                    (data.date_range.last().map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default())
                }
                button class="btn btn-sm btn-primary"
                    _="on click add .is-open to #reschedule-modal" {
                    "重新排程"
                }
            }

            // Gantt 表格
            div class="gantt-table-wrap" {
                table class="gantt-table" {
                    thead {
                        tr {
                            th class="gantt-row-header" { "工作中心" }
                            @for date in &data.date_range {
                                th class="gantt-col-header" {
                                    div { (date.format("%m/%d").to_string()) }
                                    div class="gantt-day-name" { (weekday_cn(date.weekday())) }
                                }
                            }
                        }
                    }
                    tbody {
                        @for wc in &data.work_centers {
                            tr {
                                td class="gantt-row-header" {
                                    div class="wc-name" { (wc.name) }
                                    div class="wc-code mono muted" { (wc.code) }
                                }
                                @for date in &data.date_range {
                                    td class="gantt-cell" {
                                        (render_gantt_cell(wc.id, *date, &data.items))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 重新排程弹窗
        div class="modal-overlay" id="reschedule-modal" {
            div class="modal modal-sm" {
                div class="modal-head" {
                    h2 { "重新排程" }
                    button _="on click remove .is-open from #reschedule-modal" { "×" }
                }
                form class="modal-body"
                    hx-post="/admin/mes/schedule/run"
                    hx-redirect="/admin/mes/schedule?view=gantt" {
                    div class="form-field" {
                        label { "排程策略" }
                        div class="radio-group" {
                            label { input type="radio" name="strategy" value="forward" checked {}; " 前向排程" }
                        }
                    }
                    p class="modal-desc muted" { "将对所有待排产工单重新排程。" }
                }
                div class="modal-foot" {
                    button class="btn btn-default" type="button"
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

fn render_gantt_cell(wc_id: i64, date: chrono::NaiveDate, items: &[GanttScheduleItem]) -> Markup {
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
                       title=(format!("{} {}", item.wo_doc_number, item.product_name)) {
                        div class="gantt-block-title" {
                            (item.wo_doc_number) " " (item.process_name)
                        }
                        div class="gantt-block-batch muted" {
                            (item.batch_no.as_deref().unwrap_or("")) " · " (crate::utils::fmt_qty(item.qty)) "件"
                        }
                    }
                }
            }
        }
    }
}

fn weekday_cn(wd: chrono::Weekday) -> &'static str {
    use chrono::Weekday;
    match wd {
        Weekday::Mon => "周一",
        Weekday::Tue => "周二",
        Weekday::Wed => "周三",
        Weekday::Thu => "周四",
        Weekday::Fri => "周五",
        Weekday::Sat => "周六",
        Weekday::Sun => "周日",
    }
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/mes_schedule_board.rs
git commit -m "feat: add dual-view schedule board with Gantt matrix"
```

---

## Task 4: 增强 Kanban 卡片

**Files:**
- Modify: `abt-web/src/pages/mes_schedule_board.rs` (kanban_card 函数)

- [ ] **Step 1: 在 kanban_card 中添加工作中心和计划时间**

在 `kanban_card` 函数的 `html!` 块中，在工单编号后添加：

```rust
            @if let Some(wc_name) = card.work_center_name.as_deref() {
                div class="kanban-card-meta" {
                    span class="muted" { "📍 " (wc_name) }
                }
            }
            @if let Some(start) = card.scheduled_start {
                div class="kanban-card-meta" {
                    span class="muted mono" {
                        "🕐 " (start.format("%m/%d %H:%M"))
                    }
                }
            }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_schedule_board.rs
git commit -m "feat: enhance kanban card with work center and schedule info"
```

---

## Task 5: 新增排程触发路由

**Files:**
- Modify: `abt-web/src/routes/mes_batch.rs`

- [ ] **Step 1: 添加 ScheduleRunPath 和路由**

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/schedule/run")]
pub struct ScheduleRunPath;

// 在 router() 中添加:
.route(ScheduleRunPath::PATH, post(mes_schedule_board::post_schedule_run))
```

- [ ] **Step 2: 在 mes_schedule_board.rs 中添加 post_schedule_run handler**

```rust
#[require_permission("WORK_ORDER", "update")]
pub async fn post_schedule_run(
    _path: ScheduleRunPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ScheduleRunForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // 调用排程算法
    state.production_plan_service()
        .schedule(&service_ctx, &mut tx, abt_core::mes::production_plan::model::ScheduleReq::default())
        .await?;

    tx.commit().await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    Ok(axum::response::Response::builder()
        .header("HX-Redirect", "/admin/mes/schedule?view=gantt")
        .body(axum::body::Body::empty()).unwrap())
}

#[derive(Debug, serde::Deserialize)]
pub struct ScheduleRunForm {
    #[serde(default)]
    pub strategy: Option<String>,
}
```

注意：需要导入 `ScheduleRunPath` 从 routes 模块。`ScheduleReq` 的具体形式需要与 abt-core 中已有的排程接口匹配。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/routes/mes_batch.rs abt-web/src/pages/mes_schedule_board.rs
git commit -m "feat: add schedule run endpoint"
```

---

## Task 6: CSS — Gantt / view-toggle

**Files:**
- Modify: `static/base.css`

- [ ] **Step 1: 添加样式**

```css
/* ── View Toggle ── */
.view-toggle {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border, #e5e7eb);
    margin-bottom: 16px;
}
.tab-btn {
    padding: 8px 16px;
    border: none;
    background: none;
    cursor: pointer;
    color: #999;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
    font-size: 14px;
}
.tab-btn.active {
    color: var(--primary, #165dff);
    border-bottom-color: var(--primary, #165dff);
    font-weight: 500;
}
.tab-btn:hover { color: #333; }

/* ── Gantt Table ── */
.gantt-table-wrap {
    overflow-x: auto;
    border: 1px solid #e5e7eb;
    border-radius: 6px;
}
.gantt-table {
    width: 100%;
    border-collapse: collapse;
    min-width: 800px;
}
.gantt-table th,
.gantt-table td {
    border: 1px solid #e5e7eb;
    padding: 0;
    vertical-align: top;
}
.gantt-row-header {
    width: 140px;
    min-width: 140px;
    padding: 8px 12px !important;
    background: #fafafa;
    font-weight: 500;
}
.gantt-col-header {
    text-align: center;
    padding: 8px !important;
    background: #fafafa;
    font-size: 13px;
}
.gantt-day-name { color: #999; font-size: 11px; }
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
    color: #ddd;
}

/* ── Gantt 排程块 ── */
.gantt-block {
    display: block;
    padding: 4px 8px;
    margin-bottom: 4px;
    border-radius: 4px;
    text-decoration: none;
    color: #333;
    font-size: 12px;
    transition: transform 0.15s;
}
.gantt-block:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}
.gantt-block.status-released {
    background: rgba(0, 122, 255, 0.08);
    border-left: 3px solid #007aff;
}
.gantt-block.status-in-production {
    background: rgba(82, 196, 26, 0.08);
    border-left: 3px solid #52c41a;
}
.gantt-block.status-other {
    background: rgba(255, 159, 67, 0.08);
    border-left: 3px solid #ff9f43;
}
.gantt-block-title { font-weight: 500; }
.gantt-block-batch { font-size: 11px; }

/* ── Gantt Controls ── */
.gantt-controls {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
}
.gantt-range-label {
    font-size: 14px;
    font-weight: 500;
}

/* ── Hidden ── */
.hidden { display: none !important; }
```

- [ ] **Step 2: Commit**

```bash
git add static/base.css
git commit -m "style: add Gantt and view-toggle CSS"
```

---

## Task 7: cargo clippy 最终验证

- [ ] **Step 1: 运行 clippy（两个 crate）**

Run: `cargo clippy -p abt-core -p abt-web 2>&1`
Expected: 零 error

- [ ] **Step 2: 修复所有 error**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "fix: resolve clippy errors for schedule board enhancement"
```

---

## Task 8: E2E 测试 — 排程看板增强

**验证目标：** 视图切换、Gantt 矩阵渲染、Kanban 卡片增强、重新排程弹窗。

- [ ] **Step 1: 登录**

```bash
agent-browser --cdp 9222 open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

- [ ] **Step 2: 打开排程看板**

```bash
agent-browser --cdp 9222 open https://localhost:8000/admin/mes/schedule
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 页面标题 "排程看板"
- 统计行显示 5 个 stat-card（活跃工单/待排产/进行中/待入库/已完成）
- 存在视图切换按钮："状态看板" + "工作中心排程"
- 默认显示 Kanban 视图

- [ ] **Step 3: 测试 Kanban 视图**

```bash
agent-browser snapshot -i
```

验证：
- Kanban 四列正常渲染（待排产/进行中/待入库/已完成）
- 卡片显示批次编号、产品名、完工进度条
- 卡片显示当前工序信息

- [ ] **Step 4: 切换到 Gantt 视图**

```bash
agent-browser click @e<gantt_tab_button>
agent-browser wait 500
agent-browser snapshot -i
```

验证：
- Kanban 视图隐藏
- Gantt 表格显示
- 行 = 工作中心，列 = 日期
- 每个单元格显示排程块或 "—"
- 存在日期范围标签
- 存在 "重新排程" 按钮

- [ ] **Step 5: 测试排程块交互**

```bash
agent-browser snapshot -i
```

验证：
- 排程块有颜色编码（蓝色=Released / 绿色=InProduction）
- 排程块显示工单编号 + 工序名称
- 排程块可点击（有 href 指向工单详情）

- [ ] **Step 6: 测试重新排程弹窗**

```bash
agent-browser click @e<reschedule_button>
agent-browser wait 500
agent-browser snapshot -i
```

验证：
- 弹窗显示，标题 "重新排程"
- 存在 "取消" 和 "开始排程" 按钮
- 存在排程策略选项

取消弹窗：

```bash
agent-browser click @e<cancel_reschedule_button>
agent-browser wait 500
```

- [ ] **Step 7: 切换回 Kanban 视图**

```bash
agent-browser click @e<kanban_tab_button>
agent-browser wait 500
agent-browser snapshot -i
```

验证：Kanban 视图重新显示，Gantt 隐藏。

- [ ] **Step 8: 检查控制台错误**

```bash
agent-browser console --clear
agent-browser --cdp 9222 open https://localhost:8000/admin/mes/schedule?view=gantt
agent-browser wait 1000
agent-browser errors
```

验证：无 JavaScript 错误。

- [ ] **Step 9: 记录测试结果**

---

## Self-Review Checklist

- [ ] ScheduleCard 有 work_center_name / scheduled_start / scheduled_end 字段
- [ ] GanttViewData / GanttWorkCenter / GanttScheduleItem 模型定义
- [ ] MesDashboardService 有 get_gantt_view 方法
- [ ] 视图切换在 Kanban 和 Gantt 之间无缝切换
- [ ] Gantt 表格按工作中心行 × 日期列渲染
- [ ] 排程块颜色按工单状态区分
- [ ] 重新排程弹窗可触发排程
- [ ] Kanban 卡片显示工作中心 + 计划时间
- [ ] CSS 有 gantt / view-toggle 样式
- [ ] cargo clippy 零 error（abt-core + abt-web）
- [ ] E2E 测试全部通过
