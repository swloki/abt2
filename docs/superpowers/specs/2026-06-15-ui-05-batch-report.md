# UI-05: 流转卡与报工增强

> 核心改动牵涉：文档④（报工完善）— actual_start/end 维护、工序 Completed 状态、suspend/scrap reason 审计、N+1 修复
> Odoo 参考：`mrp.workorder`（工序进度）+ `mrp.workcenter.productivity`（工时记录）

## 1. 目标

流转卡详情页和报工详情页需展示核心改动后的工序执行进度、时间戳、暂停/报废原因审计。

## 2. 当前状态

### 流转卡详情 (`mes_batch_detail.rs`)

已有：批次信息卡 + 工序执行进度表 + 报工记录列表 + 审计日志

缺失：
- actual_start / actual_end 时间戳（info 卡片无此字段）
- 工序执行进度表无 Completed 状态标识
- 暂停/报废原因不在审计日志中显示（核心已修复，但 UI 未展示）

### 报工详情 (`mes_report_detail.rs`)

已有：报工信息 + 工资计算

缺失：
- 工资计算明细不显示关联工序信息（work_center / unit_price / standard_time）
- 工序进度上下文缺失

## 3. Odoo 参考

### Odoo `mrp.workorder` 表单

```
┌── Work Order: 注塑 (Operation 1) ────────────────────────┐
│                                                           │
│  ┌── 进度 ─────────────────────────────────────────────┐ │
│  │ Production: 95/100                                  │ │
│  │ ████████████████████████████░░░ 95%                 │ │
│  │ State: ● Done                                       │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                           │
│  ┌── 时间 ─────────────────────────────────────────────┐ │
│  │ Planned Duration: 2h    Real Duration: 1.8h         │ │
│  │ Started: 06/15 08:30    Finished: 06/15 10:18      │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                           │
│  ┌── 工时记录 ─────────────────────────────────────────┐ │
│  │ Operator │ Start    │ End     │ Duration │ Loss      │ │
│  │ 张三     │ 08:30   │ 10:00   │ 1.5h     │ —         │ │
│  │ 李四     │ 10:00   │ 10:18   │ 0.3h     │ 换模      │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                           │
│  [Start] [Pause] [Finish] [Quality Check]                │
└───────────────────────────────────────────────────────────┘
```

### Odoo `mrp.workcenter.productivity.loss`

暂停/报废原因通过 loss 类型追踪：
- `productive` — 正常生产
- `performance` — 效率损失
- `availability` — 设备故障/缺料
- `quality` — 质量问题

**关键 Odoo 模式**：
1. **Planned vs Real Duration** — 计划工时 vs 实际工时对比
2. **Loss tracking** — 每段非生产时间记录原因
3. **Operator-level time tracking** — 按操作工分段计时

### 我们的适配

不做 Odoo 的实时计时器（SSR 架构限制）。改为报工时记录时间点，事后展示。暂停/报废原因通过审计日志 + tracing 展示。

## 4. 流转卡详情修改 (`mes_batch_detail.rs`)

### 4.1 Info 卡片 — 新增时间戳

```
┌── 批次信息 ──────────────────────────────────────────────────┐
│  批次编号: B001               状态: ● 生产中                  │
│  工单: MO-2024-001 →         产品: 电源板A                    │
│  计划数量: 100                完工数量: 0                      │
│  报废数量: 0                  不合格: 0                        │
│                                                              │
│  ── 时间 ──                                                  │
│  计划开始: 2024-06-15 08:00    实际开始: 2024-06-15 08:30     │
│  计划结束: 2024-06-15 18:00    实际结束: —（进行中）          │
│  已耗时: 6.5h / 10h (65%)                                    │
└──────────────────────────────────────────────────────────────┘
```

**实现**：

```rust
// mes_batch_detail.rs — info 卡片新增时间区块
fn batch_info_section(batch: &ProductionBatch, order: &WorkOrder) -> Markup {
    html! {
        div class="info-card" {
            div class="info-grid" {
                // ... 现有字段 ...
            }
            // ── 新增：时间区块 ──
            div class="info-section" {
                div class="info-section-title" { "生产时间" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "计划开始" }
                        span class="info-value mono" { (order.scheduled_start) }
                    }
                    div class="info-item" {
                        span class="info-label" { "实际开始" }
                        @if let Some(start) = batch.actual_start {
                            span class="info-value mono" { (fmt_dt(start)) }
                        } @else {
                            span class="info-value muted" { "—（未开始）" }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "计划结束" }
                        span class="info-value mono" { (order.scheduled_end) }
                    }
                    div class="info-item" {
                        span class="info-label" { "实际结束" }
                        @if let Some(end) = batch.actual_end {
                            span class="info-value mono" { (fmt_dt(end)) }
                        } @else {
                            span class="info-value muted" { "—（进行中）" }
                        }
                    }
                }
                // 已耗时进度
                @if let Some(start) = batch.actual_start {
                    (elapsed_progress(start, batch.actual_end, &order.scheduled_start, &order.scheduled_end))
                }
            }
        }
    }
}

fn elapsed_progress(actual_start: DateTime<Utc>, actual_end: Option<DateTime<Utc>>, planned_start: &str, planned_end: &str) -> Markup {
    let now = Utc::now();
    let end = actual_end.unwrap_or(now);
    let elapsed = end.signed_duration_since(actual_start);
    let elapsed_h = elapsed.num_minutes() as f64 / 60.0;
    html! {
        div class="elapsed-bar" {
            span class="info-label" { "已耗时" }
            span class="info-value mono" {
                (format!("{:.1}h", elapsed_h))
            }
        }
    }
}
```

### 4.2 工序执行进度表 — 新增状态列

当前列：序号 / 工序名称 / 计划量 / 已完工量 / 合格量 / 报废量 / 状态

**状态列增强**：

```rust
fn routing_step_status_badge(status: BatchRoutingStatus) -> Markup {
    let (label, class) = match status {
        BatchRoutingStatus::Pending => ("待加工", "status-pill status-draft"),
        BatchRoutingStatus::InProgress => ("加工中", "status-pill status-info"),
        BatchRoutingStatus::Completed => ("已完成", "status-pill status-completed"),
    };
    html! { span class=(class) { (label) } }
}
```

**进度条**（在"已完工量"列内嵌）：

```rust
td {
    div class="step-progress" {
        span class="mono" { (fmt_qty(r.completed_qty)) " / " (fmt_qty(r.planned_qty)) }
        div class="mini-progress-bar" {
            div class="mini-progress-fill"
                 style=(format!("width: {}%", completion_pct(r.completed_qty, r.planned_qty)))
            {}
        }
    }
}
```

### 4.3 暂停/报废原因审计

核心改动后，suspend/scrap 的 reason 已通过 `tracing::info!` 记录。审计日志中会有 `changes` 字段包含 reason。

**审计日志 Tab 增强**：

```rust
fn tab_log(logs: &[AuditLog]) -> Markup {
    html! {
        div class="data-card" {
            table class="data-table" {
                thead {
                    tr {
                        th { "时间" }
                        th { "操作" }
                        th { "操作人" }
                        th { "详情" }   // ← 新增列
                    }
                }
                tbody {
                    @for log in logs {
                        tr {
                            td class="mono" { (fmt_dt(log.created_at)) }
                            td { (audit_action_label(log.action)) }
                            td class="mono" { "#" (log.operator_id) }
                            td {
                                @if let Some(changes) = &log.changes {
                                    (render_changes(changes))
                                } @else {
                                    span class="muted" { "—" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_changes(changes: &serde_json::Value) -> Markup {
    if let Some(obj) = changes.as_object() {
        html! {
            div class="audit-changes" {
                @for (key, val) in obj {
                    div class="change-row" {
                        span class="change-key" { (key) ":" }
                        span class="change-val mono" { (val) }
                    }
                }
            }
        }
    } else {
        html! { span class="mono" { (changes) } }
    }
}
```

效果：

```
时间               │ 操作       │ 操作人  │ 详情
2024-06-15 10:30   │ 暂停       │ #5     │ reason: 设备故障维修
2024-06-15 11:00   │ 恢复       │ #5     │ reason: 维修完成
2024-06-15 14:00   │ 报废       │ #5     │ reason: 质量不达标，报废 10 件
2024-06-15 14:30   │ 报工确认   │ #5     │ step: 2, qty: 90, qualified: 88
```

## 5. 报工详情修改 (`mes_report_detail.rs`)

### 5.1 工资计算明细增强

当前：显示操作工 + 工资金额。

改为：展示完整计算链路。

```
┌── 工资计算 ──────────────────────────────────────────────────┐
│                                                              │
│  报工数量:    100                                             │
│  合格数量:    95                                              │
│  工资类型:    计件                                            │
│                                                              │
│  ── 计算明细 ──                                               │
│  工序:        组装 (Step 2)                                   │
│  工作中心:    WC002 - 组装线B                                 │
│  计件单价:    ¥1.00/件                                       │
│  计算公式:    合格量(95) × 单价(¥1.00) = ¥95.00              │
│                                                              │
│  工资合计:    ¥95.00                                          │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**实现**：

```rust
fn wage_calc_detail(
    report: &WorkReport,
    routing: &WorkOrderRouting,
    wc_name: &str,
) -> Markup {
    let wage = report.qualified_qty * routing.unit_price.unwrap_or(Decimal::ZERO);
    html! {
        div class="data-card" {
            div class="data-card-head" {
                h3 { "工资计算" }
            }
            div class="info-grid" {
                div class="info-item" {
                    span class="info-label" { "报工数量" }
                    span class="info-value mono" { (fmt_qty(report.completed_qty)) }
                }
                div class="info-item" {
                    span class="info-label" { "合格数量" }
                    span class="info-value mono" { (fmt_qty(report.qualified_qty)) }
                }
                div class="info-item" {
                    span class="info-label" { "工资类型" }
                    span class="info-value" { "计件" }
                }
            }
            // 计算明细
            div class="calc-detail" {
                div class="calc-row" {
                    span class="calc-label" { "工序" }
                    span class="calc-value" {
                        (routing.process_name.as_str())
                        " (Step " (routing.step_no) ")"
                    }
                }
                div class="calc-row" {
                    span class="calc-label" { "工作中心" }
                    span class="calc-value" { (wc_name) }
                }
                div class="calc-row" {
                    span class="calc-label" { "计件单价" }
                    span class="calc-value mono" {
                        (fmt_money(routing.unit_price.unwrap_or(Decimal::ZERO))) "/件"
                    }
                }
                div class="calc-formula" {
                    code {
                        "合格量(" (fmt_qty(report.qualified_qty)) ")"
                        " × 单价(" (fmt_money(routing.unit_price.unwrap_or(Decimal::ZERO))) ")"
                        " = " strong { (fmt_money(wage)) }
                    }
                }
            }
            div class="amount-summary" {
                div class="amount-row total" {
                    span { "工资合计" }
                    span class="mono amount-value" { (fmt_money(wage)) }
                }
            }
        }
    }
}
```

### 5.2 工序上下文展示

在报工详情顶部新增"所属工序"信息条：

```rust
fn routing_context_bar(routing: &WorkOrderRouting, batch_no: &str) -> Markup {
    html! {
        div class="context-bar" {
            span class="context-item" {
                (icon::workflow_icon("w-4 h-4"))
                (routing.process_name.as_str())
                " (Step " (routing.step_no) ")"
            }
            span class="context-sep" { "·" }
            span class="context-item" { "批次 " (batch_no) }
            span class="context-sep" { "·" }
            span class="context-item" {
                @if routing.is_inspection_point {
                    span class="tag-chip tag-warning" { "报检点" }
                }
            }
        }
    }
}
```

## 6. 报工列表修改 (`mes_report_list.rs`)

列表新增列：工序名称（当前缺失）。

```rust
// 列表新增"工序"列
th { "工序" }
td { (report.process_name.as_deref().unwrap_or("—")) }
```

## 7. CSS 新增

```css
/* mini-progress-bar — 工序表内嵌进度条 */
.mini-progress-bar {
    height: 4px;
    background: var(--gray-100);
    border-radius: var(--radius-pill);
    margin-top: 4px;
    overflow: hidden;
}
.mini-progress-fill {
    height: 100%;
    background: var(--primary);
    border-radius: var(--radius-pill);
    transition: width 0.3s ease;
}

/* audit-changes — 审计日志变更详情 */
.audit-changes {
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.change-row {
    display: flex;
    gap: 4px;
    font-size: var(--text-xs);
}
.change-key { color: var(--muted); }
.change-val { color: var(--text); }

/* calc-detail — 工资计算明细 */
.calc-detail {
    padding: 12px 16px;
    background: var(--gray-50);
    border-radius: var(--radius);
    margin: 12px 0;
}
.calc-row {
    display: flex;
    gap: 12px;
    padding: 4px 0;
}
.calc-label {
    width: 80px;
    color: var(--muted);
    font-size: var(--text-sm);
}
.calc-value {
    color: var(--text);
    font-size: var(--text-sm);
}
.calc-formula {
    margin-top: 8px;
    padding: 8px 12px;
    background: var(--white);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
}

/* context-bar — 工序上下文条 */
.context-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: var(--gray-50);
    border-radius: var(--radius);
    margin-bottom: 12px;
}
.context-item {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--text-sm);
}
.context-sep { color: var(--muted); }
```

## 8. 实现步骤

### 流转卡详情

1. `mes_batch_detail.rs`:
   - info 卡片新增"生产时间"区块（actual_start / actual_end）
   - 工序进度表状态列改用 badge + mini progress bar
   - 审计日志 Tab 新增"详情"列（渲染 changes JSON）

### 报工详情

2. `mes_report_detail.rs`:
   - 新增 `routing_context_bar` — 工序上下文条
   - 工资计算区块增强 — 展示计算链路
   - handler 预加载 routing + work_center 名称

### 报工列表

3. `mes_report_list.rs`:
   - 新增"工序"列

### CSS

4. `base.css` — 加 mini-progress / audit-changes / calc-detail / context-bar

## 9. 验收标准

- [ ] 流转卡详情显示 actual_start / actual_end
- [ ] 工序进度表有 Completed 状态 badge + mini progress bar
- [ ] 审计日志显示暂停/报废原因
- [ ] 报工详情显示工资计算明细（工序 + 工作中心 + 计件单价 + 公式）
- [ ] 报工列表有工序列
- [ ] cargo clippy 零错误
