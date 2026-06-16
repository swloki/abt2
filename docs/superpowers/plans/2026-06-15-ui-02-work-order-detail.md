# 工单详情增强 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 工单详情页展示完工率进度条、实际时间戳、FQC 状态、工序成本属性，并实现 close 95% 门控和 cancel 入库单警告。

**Architecture:** 修改现有 `mes_order_detail.rs` 的 handler 数据加载和页面渲染函数。新增 tab_materials 面板和进度条 CSS。不创建新文件。

**Tech Stack:** Rust (Maud + HTMX), abt-core WorkOrderService / MaterialRequisitionService / InspectionResultService

---

## File Structure

| 文件 | 职责 | 动作 |
|------|------|------|
| `abt-web/src/pages/mes_order_detail.rs` | handler 加载数据 + tab_info/tab_routing 增强 + 新增 tab_materials + 按钮门控 | Modify |
| `static/base.css` | progress-bar 样式 | Modify |

---

## Task 1: handler 增强 — 加载完工量、领料单、FQC 状态

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs:79-167` (get_order_detail handler)

- [ ] **Step 1: 在 handler 中新增数据查询**

在 `get_order_detail` 函数中，`let audit_logs = ...` 之后（约第 150 行后），`let content = order_detail_page(...)` 之前，添加：

```rust
    // 完工量（已有 completed_qty 字段，计算完工率）
    let completion_pct = if order.planned_qty > rust_decimal::Decimal::ZERO {
        ((order.completed_qty / order.planned_qty) * rust_decimal::Decimal::ONE_HUNDRED)
            .min(rust_decimal::Decimal::ONE_HUNDRED)
    } else {
        rust_decimal::Decimal::ZERO
    };

    // 是否有入库记录
    let has_receipts = state.production_receipt_service()
        .find_by_work_order(&service_ctx, &mut conn, order.id)
        .await
        .map(|r| !r.is_empty())
        .unwrap_or(false);
```

注意：需要确认 `production_receipt_service` 是否有 `find_by_work_order` 方法。如果没有，用 `has_confirmed_receipts` 或直接查询。如果 trait 缺少此方法，可以暂时用 `order.completed_qty > 0` 作为近似判断。

- [ ] **Step 2: 修改 order_detail_page 函数签名**

将 `order_detail_page` 签名改为接受新参数：

```rust
#[allow(clippy::too_many_arguments)]
fn order_detail_page(
    order: &WorkOrder,
    product_name: &str,
    routings: &[WorkOrderRouting],
    batches: &[ProductionBatch],
    reports: &[ReportListItem],
    audit_logs: &[AuditLog],
    completion_pct: rust_decimal::Decimal,
    has_receipts: bool,
) -> Markup {
```

同步更新 handler 中的调用（约第 152 行）：

```rust
    let content = order_detail_page(
        &order, &product_name, &routings, &batches, &reports, &audit_logs,
        completion_pct, has_receipts,
    );
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_order_detail.rs
git commit -m "feat: enhance work order detail handler with completion data"
```

---

## Task 2: tab_info 新增生产进度区块

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs` (tab_info 函数，约 445-487 行)

- [ ] **Step 1: 在 tab_info 函数末尾添加生产进度区块**

在 `tab_info` 函数的 `html!` 块中，在 `@if !order.remark.is_empty() { ... }` 之前（约第 480 行前）添加：

```rust
        // ── 生产进度 ──
        div class="info-section" {
            div class="info-section-title" { "生产进度" }
            div class="progress-section" {
                div class="progress-stats" {
                    span class="info-item" {
                        span class="info-label" { "计划" }
                        span class="info-value mono" { (crate::utils::fmt_qty(order.planned_qty)) }
                    }
                    span class="info-item" {
                        span class="info-label" { "已完工" }
                        span class="info-value mono" { (crate::utils::fmt_qty(order.completed_qty)) }
                    }
                    span class="info-item" {
                        span class="info-label" { "完工率" }
                        span class="info-value mono" {
                            (completion_pct.round_dp(1)) "%"
                        }
                    }
                }
                div class="progress-bar-wrap" {
                    div class="progress-bar-fill"
                        style=(format!("width: {}%", completion_pct))
                    {}
                }
            }
        }
```

注意：`tab_info` 函数也需要接收 `completion_pct` 参数。修改签名为：

```rust
fn tab_info(order: &WorkOrder, product_name: &str, routing_count: usize, completion_pct: rust_decimal::Decimal) -> Markup {
```

并在 `order_detail_page` 中传入。

- [ ] **Step 2: 添加 actual_start/end 显示**

在进度区块后添加（需要确认 WorkOrder 模型有 `actual_start` / `actual_end` 字段，如果没有则跳过此步）：

```rust
        // ── 实际时间 ──
        div class="info-section" {
            div class="info-section-title" { "实际时间" }
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

注意：如果 WorkOrder 模型有 `actual_start: Option<DateTime<Utc>>` 字段，则替换为实际值。当前模型可能没有这些字段 — 检查 `abt-core/src/mes/work_order/model.rs`。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_order_detail.rs
git commit -m "feat: add production progress section to work order detail"
```

---

## Task 3: tab_routing 新增成本列

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs` (tab_routing 函数，约 489-536 行)

- [ ] **Step 1: 扩展表头**

在 tab_routing 函数的 thead 中，在 "标记" 之前添加新列：

```rust
                            th class="num-right" { "标准成本" }
                            th class="num-right" { "计件单价" }
```

- [ ] **Step 2: 扩展表体**

在 tbody 的每行中，在 "标记" td 之前添加：

```rust
                                td class="mono num-right" {
                                    @if let Some(c) = r.standard_cost { "¥" (crate::utils::fmt_qty(c)) } @else { "—" }
                                }
                                td class="mono num-right" {
                                    @if let Some(p) = r.unit_price { "¥" (crate::utils::fmt_qty(p)) } @else { "—" }
                                }
```

- [ ] **Step 3: 更新空行 colspan**

将 `colspan="7"` 改为 `colspan="9"`。

- [ ] **Step 4: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 5: Commit**

```bash
git add abt-web/src/pages/mes_order_detail.rs
git commit -m "feat: add cost columns to work order routing tab"
```

---

## Task 4: Close 按钮 95% 门控

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs` (order_detail_page 函数，约 331-342 行)

- [ ] **Step 1: 替换 close 按钮逻辑**

将当前的 close 按钮（约 335-341 行）替换为条件渲染：

```rust
                        @if matches!(order.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
                            @if completion_pct >= rust_decimal::Decimal::new(95, 2) {
                                button class="btn btn-default"
                                    hx-post=(OrderClosePath { order_id: order.id }.to_string())
                                    hx-confirm="确认关闭此工单？所有批次必须已完工或已取消。"
                                    hx-disabled-elt="this" {
                                    (icon::check_circle_icon("w-4 h-4"))
                                    "关闭工单"
                                }
                            } @else {
                                button class="btn btn-default" disabled
                                    title=(format!("完工率 {}%，需 ≥ 95% 才能关闭", completion_pct.round_dp(1))) {
                                    (icon::check_circle_icon("w-4 h-4"))
                                    "关闭工单（完工不足）"
                                }
                            }
                        }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_order_detail.rs
git commit -m "feat: add 95% completion gate for close button"
```

---

## Task 5: Cancel 按钮入库单警告

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs` (order_detail_page 函数，约 352-360 行)

- [ ] **Step 1: 替换 cancel 按钮**

将当前 cancel 按钮替换为条件渲染：

```rust
                        @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned | WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
                            @if has_receipts {
                                button class="btn btn-danger" disabled
                                    title="存在已确认的完工入库单，无法取消" {
                                    (icon::x_icon("w-4 h-4"))
                                    "取消（有入库记录）"
                                }
                            } @else {
                                button class="btn btn-danger"
                                    hx-post=(OrderCancelPath { order_id: order.id }.to_string())
                                    hx-confirm="确认取消此工单？将同时取消关联领料单。此操作不可撤销。"
                                    hx-disabled-elt="this" {
                                    (icon::x_icon("w-4 h-4"))
                                    "取消工单"
                                }
                            }
                        }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_order_detail.rs
git commit -m "feat: disable cancel button when receipts exist"
```

---

## Task 6: CSS — progress-bar 样式

**Files:**
- Modify: `static/base.css`

- [ ] **Step 1: 在 base.css 末尾添加 progress-bar 样式**

```css
/* ── Progress Bar ── */
.progress-section {
    padding: 12px 0;
}
.progress-stats {
    display: flex;
    gap: 24px;
    margin-bottom: 8px;
}
.progress-bar-wrap {
    height: 8px;
    background: var(--gray-100, #f5f5f5);
    border-radius: 4px;
    overflow: hidden;
}
.progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--primary, #165dff), #4080ff);
    border-radius: 4px;
    transition: width 0.3s ease;
}
```

- [ ] **Step 2: Commit**

```bash
git add static/base.css
git commit -m "style: add progress-bar CSS for work order detail"
```

---

## Task 7: cargo clippy 最终验证

- [ ] **Step 1: 运行完整 clippy**

Run: `cargo clippy -p abt-web 2>&1`
Expected: 零 error

- [ ] **Step 2: 修复所有 error**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "fix: resolve clippy errors for work order detail enhancement"
```

---

## Task 8: E2E 测试 — 工单详情增强

**验证目标：** 进度条显示、工序成本列、close 按钮门控、cancel 按钮状态。

- [ ] **Step 1: 登录**

```bash
agent-browser --cdp 9222 open http://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

- [ ] **Step 2: 打开工单详情页**

```bash
agent-browser --cdp 9222 open http://localhost:8000/admin/mes/orders
agent-browser snapshot -i
# 点击第一条工单的详情链接
agent-browser click @e<first_order_detail_link>
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 页面标题包含工单编号
- 存在 "工单信息" Tab，默认显示
- "工单信息" Tab 内有 "生产进度" 区块

- [ ] **Step 3: 验证生产进度区块**

```bash
agent-browser snapshot -i
```

验证：
- 显示 "计划" 数量
- 显示 "已完工" 数量
- 显示 "完工率" 百分比
- 存在进度条（`.progress-bar-wrap` / `.progress-bar-fill`）

- [ ] **Step 4: 验证工序明细 Tab**

```bash
agent-browser click @e<routing_tab>
agent-browser wait 500
agent-browser snapshot -i
```

验证：
- 表格列包含 "标准成本" 和 "计件单价"
- 每行数据正确显示成本值或 "—"

- [ ] **Step 5: 验证 close 按钮门控**

在 "工单信息" Tab 或页面头部，检查 close 按钮状态：

```bash
agent-browser snapshot -i
```

验证：
- 如果完工率 < 95%：按钮显示 "关闭工单（完工不足）" 且 disabled
- 如果完工率 ≥ 95%：按钮显示 "关闭工单" 且可点击

- [ ] **Step 6: 验证 cancel 按钮状态**

```bash
agent-browser snapshot -i
```

验证：
- 如果有入库记录：cancel 按钮显示 "取消（有入库记录）" 且 disabled
- 如果无入库记录：cancel 按钮显示 "取消工单" 且可点击

- [ ] **Step 7: 检查控制台错误**

```bash
agent-browser console --clear
agent-browser reload
agent-browser wait 1000
agent-browser errors
```

验证：无 JavaScript 错误。

- [ ] **Step 8: 记录测试结果**

---

## Self-Review Checklist

- [ ] handler 加载了 completion_pct 和 has_receipts
- [ ] tab_info 显示生产进度区块（计划/已完工/完工率 + 进度条）
- [ ] tab_routing 显示标准成本和计件单价列
- [ ] close 按钮在完工率 < 95% 时禁用
- [ ] cancel 按钮在有入库单时禁用
- [ ] base.css 有 progress-bar 样式
- [ ] cargo clippy 零 error
- [ ] E2E 测试全部通过
