# 流转卡与报工增强 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 流转卡详情展示 actual_start/end 时间戳、工序 Completed 状态 badge、审计日志 reason。报工详情展示工资计算明细。

**Architecture:** 修改 `mes_batch_detail.rs` 和 `mes_report_detail.rs`，新增 CSS 样式。

**Tech Stack:** Rust (Maud + HTMX), abt-core ProductionBatchService / WorkReportService

---

## File Structure

| 文件 | 职责 | 动作 |
|------|------|------|
| `abt-web/src/pages/mes_batch_detail.rs` | info 卡片加时间 + 工序 badge + 审计详情列 | Modify |
| `abt-web/src/pages/mes_report_detail.rs` | 工资计算明细 + 工序上下文 | Modify |
| `abt-web/src/pages/mes_report_list.rs` | 加工序列 | Modify |
| `static/base.css` | mini-progress / audit-changes / calc-detail | Modify |

---

## Task 1: 流转卡详情 — 时间戳区块

**Files:**
- Modify: `abt-web/src/pages/mes_batch_detail.rs`

- [ ] **Step 1: 在 info 卡片中添加生产时间区块**

在 `mes_batch_detail.rs` 中找到批次信息卡片的渲染函数，在基础信息后添加：

```rust
        // ── 生产时间 ──
        div class="info-section" {
            div class="info-section-title" { "生产时间" }
            div class="info-grid" {
                div class="info-item" {
                    span class="info-label" { "实际开始" }
                    span class="info-value mono" { "—" }
                }
                div class="info-item" {
                    span class="info-label" { "实际结束" }
                    span class="info-value mono" { "—" }
                }
            }
        }
```

注意：`ProductionBatch` 模型需要检查是否有 `actual_start` / `actual_end` 字段。核心层已添加这些字段（文档④）。如果有，替换为：

```rust
                div class="info-item" {
                    span class="info-label" { "实际开始" }
                    @if let Some(start) = batch.actual_start {
                        span class="info-value mono" { (start.format("%Y-%m-%d %H:%M")) }
                    } @else {
                        span class="info-value muted" { "—（未开始）" }
                    }
                }
```

需要确认 `batch` 变量在当前渲染上下文中可用。如果不可用，需要修改 handler 将 batch 数据传入渲染函数。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_batch_detail.rs
git commit -m "feat: add production time section to batch detail"
```

---

## Task 2: 工序进度表 — 状态 badge + mini progress bar

**Files:**
- Modify: `abt-web/src/pages/mes_batch_detail.rs`

- [ ] **Step 1: 找到工序进度表渲染函数**

搜索 `mes_batch_detail.rs` 中的 routing progress 或 batch routing 表格。

- [ ] **Step 2: 在状态列中用 badge 替代纯文本**

将工序状态列替换为 badge 渲染：

```rust
// 状态列
td {
    @if step.completed_qty >= step.batch_qty && step.batch_qty > rust_decimal::Decimal::ZERO {
        span class="status-pill status-completed" { "✓ 已完成" }
    } @else if step.completed_qty > rust_decimal::Decimal::ZERO {
        span class="status-pill status-progress" { "加工中" }
    } @else {
        span class="status-pill status-draft" { "待加工" }
    }
}
```

- [ ] **Step 3: 在"已完工量"列内嵌 mini progress bar**

```rust
td {
    div class="step-progress" {
        span class="mono" {
            (crate::utils::fmt_qty(step.completed_qty))
            " / "
            (crate::utils::fmt_qty(step.batch_qty))
        }
        div class="mini-progress-bar" {
            @if step.batch_qty > rust_decimal::Decimal::ZERO {
                div class="mini-progress-fill"
                    style=(format!("width: {}%",
                        (step.completed_qty / step.batch_qty * rust_decimal::Decimal::ONE_HUNDRED)
                        .min(rust_decimal::Decimal::ONE_HUNDRED)))
                {}
            }
        }
    }
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/mes_batch_detail.rs
git commit -m "feat: add status badge and mini progress bar to batch routing"
```

---

## Task 3: 审计日志 Tab — 新增详情列

**Files:**
- Modify: `abt-web/src/pages/mes_batch_detail.rs`

- [ ] **Step 1: 找到审计日志 Tab 的渲染**

如果 `mes_batch_detail.rs` 有审计日志 Tab，找到其表头和表体。如果没有，跳过此 Task（审计日志在工单详情页已有，流转卡详情可能不单独展示）。

- [ ] **Step 2: 在审计日志表格中新增"详情"列**

表头添加 `th { "详情" }`，表体添加：

```rust
td {
    @if let Some(changes) = &log.changes {
        @if let Some(obj) = changes.as_object() {
            div class="audit-changes" {
                @for (key, val) in obj {
                    div class="change-row" {
                        span class="change-key" { (key) ": " }
                        span class="change-val mono" { (val) }
                    }
                }
            }
        } @else {
            span class="mono" { (changes) }
        }
    } @else {
        span class="muted" { "—" }
    }
}
```

需要确认审计日志变量名和字段。如果流转卡详情页没有审计日志 Tab，此 Task 可以跳过。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_batch_detail.rs
git commit -m "feat: add changes column to batch audit log"
```

---

## Task 4: 报工详情 — 工资计算明细

**Files:**
- Modify: `abt-web/src/pages/mes_report_detail.rs`

- [ ] **Step 1: 找到报工详情页的工资计算区域**

读取 `mes_report_detail.rs`，找到现有的工资或 wage 渲染部分。

- [ ] **Step 2: 增强工资计算区块**

在报工详情页中，将工资计算区块替换为更详细的展示：

```rust
// 工资计算明细卡片
div class="info-card" {
    div class="info-section-title" { "工资计算" }
    div class="info-grid" {
        div class="info-item" {
            label { "报工数量" }
            span class="mono" { (crate::utils::fmt_qty(report.completed_qty)) }
        }
        div class="info-item" {
            label { "合格数量" }
            span class="mono" { (crate::utils::fmt_qty(report.qualified_qty)) }
        }
        div class="info-item" {
            label { "工资类型" }
            span { "计件" }
        }
    }
    div class="calc-detail" {
        div class="calc-row" {
            span class="calc-label" { "工序" }
            span class="calc-value" {
                (report.process_name.as_deref().unwrap_or("—"))
            }
        }
        div class="calc-row" {
            span class="calc-label" { "计件单价" }
            span class="calc-value mono" {
                @if let Some(price) = report.unit_price {
                    "¥" (crate::utils::fmt_qty(price)) "/件"
                } @else { "—" }
            }
        }
        div class="calc-formula" {
            code {
                "合格量(" (crate::utils::fmt_qty(report.qualified_qty)) ")"
                " × 单价("
                @if let Some(price) = report.unit_price {
                    "¥" (crate::utils::fmt_qty(price))
                } @else { "¥0" }
                ")"
                " = "
                strong {
                    "¥"
                    (crate::utils::fmt_qty(
                        report.qualified_qty * report.unit_price.unwrap_or(rust_decimal::Decimal::ZERO)
                    ))
                }
            }
        }
    }
}
```

注意：`report` 的字段名需要与 `WorkReport` 或 `ReportListItem` 模型一致。需要确认 `process_name` 和 `unit_price` 是否在模型中。如果 `ReportListItem` 没有这些字段，需要：
1. 在 abt-core 中扩展 `ReportListItem` 添加 `process_name: Option<String>` 和 `unit_price: Option<Decimal>`
2. 或者在 handler 中额外查询 routing 信息

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_report_detail.rs
git commit -m "feat: enhance wage calculation detail in report detail"
```

---

## Task 5: 报工列表 — 新增工序列

**Files:**
- Modify: `abt-web/src/pages/mes_report_list.rs`

- [ ] **Step 1: 在列表表头添加工序列**

找到列表的 `thead`，在现有列中添加：

```rust
th { "工序" }
```

- [ ] **Step 2: 在列表表体添加工序数据**

在每行中添加：

```rust
td { (report.process_name.as_deref().unwrap_or("—")) }
```

注意：`ReportListItem` 需要有 `process_name` 字段。如果没有，需要在 abt-core 中添加。或者使用 routing step ID 显示。

- [ ] **Step 3: 更新空行 colspan**

如果有空行 placeholder，更新 colspan。

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/mes_report_list.rs
git commit -m "feat: add process column to report list"
```

---

## Task 6: CSS — mini-progress / audit-changes / calc-detail

**Files:**
- Modify: `static/base.css`

- [ ] **Step 1: 添加样式**

```css
/* ── Mini Progress Bar ── */
.step-progress {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 80px;
}
.mini-progress-bar {
    height: 4px;
    background: #f0f0f0;
    border-radius: 2px;
    overflow: hidden;
}
.mini-progress-fill {
    height: 100%;
    background: var(--primary, #165dff);
    border-radius: 2px;
    transition: width 0.3s ease;
}

/* ── Audit Changes ── */
.audit-changes {
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.change-row {
    display: flex;
    gap: 4px;
    font-size: 12px;
}
.change-key { color: #999; }
.change-val { color: #333; }

/* ── 工资计算明细 ── */
.calc-detail {
    padding: 12px 16px;
    background: #fafafa;
    border-radius: 6px;
    margin: 12px 0;
}
.calc-row {
    display: flex;
    gap: 12px;
    padding: 4px 0;
}
.calc-label {
    width: 80px;
    color: #999;
    font-size: 13px;
}
.calc-value {
    color: #333;
    font-size: 13px;
}
.calc-formula {
    margin-top: 8px;
    padding: 8px 12px;
    background: #fff;
    border-radius: 4px;
    font-family: var(--font-mono, monospace);
    font-size: 13px;
}
```

- [ ] **Step 2: Commit**

```bash
git add static/base.css
git commit -m "style: add mini-progress, audit-changes, calc-detail CSS"
```

---

## Task 7: cargo clippy 最终验证

- [ ] **Step 1: 运行 clippy**

Run: `cargo clippy -p abt-web 2>&1`
Expected: 零 error

- [ ] **Step 2: 修复所有 error**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "fix: resolve clippy errors for batch/report enhancement"
```

---

## Task 8: E2E 测试 — 流转卡与报工增强

**验证目标：** 时间戳显示、工序进度 badge、报工工资计算明细。

- [ ] **Step 1: 登录**

```bash
agent-browser --cdp 9222 open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

- [ ] **Step 2: 打开流转卡详情**

```bash
agent-browser --cdp 9222 open https://localhost:8000/admin/mes/cards
agent-browser snapshot -i
# 点击第一条流转卡
agent-browser click @e<first_batch_link>
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 存在 "生产时间" 区块
- 显示 "实际开始" 和 "实际结束" 时间（或 "—" 占位）

- [ ] **Step 3: 验证工序进度 badge**

```bash
agent-browser snapshot -i
```

验证：
- 工序进度表中有状态 badge（✓ 已完成 / 加工中 / 待加工）
- "已完工量" 列有 mini progress bar

- [ ] **Step 4: 打开报工详情**

```bash
agent-browser --cdp 9222 open https://localhost:8000/admin/mes/reports
agent-browser snapshot -i
```

验证：
- 报工列表有 "工序" 列

```bash
agent-browser click @e<first_report_link>
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 存在 "工资计算" 卡片
- 显示报工数量、合格数量、工资类型
- 显示工序名称和计件单价
- 显示计算公式（合格量 × 单价 = 工资金额）

- [ ] **Step 5: 检查控制台错误**

```bash
agent-browser console --clear
agent-browser reload
agent-browser wait 1000
agent-browser errors
```

验证：无 JavaScript 错误。

- [ ] **Step 6: 记录测试结果**

---

## Self-Review Checklist

- [ ] 流转卡详情显示 actual_start / actual_end 时间戳
- [ ] 工序进度表有状态 badge（已完成/加工中/待加工）
- [ ] 工序进度表有 mini progress bar
- [ ] 审计日志有"详情"列（如有审计 Tab）
- [ ] 报工详情显示工资计算明细（工序 + 单价 + 公式）
- [ ] 报工列表有"工序"列
- [ ] CSS 有 mini-progress / audit-changes / calc-detail 样式
- [ ] cargo clippy 零 error
- [ ] E2E 测试全部通过
