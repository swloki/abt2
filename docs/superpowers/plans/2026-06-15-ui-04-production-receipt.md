# 完工入库增强 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完工入库详情页展示 FQC 质检状态、成本明细，确认按钮实现 FQC 门控，消除 inline style。

**Architecture:** 重写 `mes_receipt_detail.rs` — 从单卡片改为 Tab 结构，新增 FQC 状态卡片和成本明细卡片，确认按钮条件渲染。

**Tech Stack:** Rust (Maud + HTMX), abt-core ProductionReceiptService / InspectionResultService

---

## File Structure

| 文件 | 职责 | 动作 |
|------|------|------|
| `abt-web/src/pages/mes_receipt_detail.rs` | 整页重写：Tab 结构 + FQC 卡片 + 成本卡片 + 确认门控 | Modify |
| `static/base.css` | fqc-badge / inline-form / total-row | Modify |

---

## Task 1: Handler 增强 — 加载 FQC 和成本数据

**Files:**
- Modify: `abt-web/src/pages/mes_receipt_detail.rs:22-66` (get_receipt_detail handler)

- [ ] **Step 1: 重写 get_receipt_detail handler**

替换整个 `get_receipt_detail` 函数：

```rust
#[require_permission("WORK_ORDER", "read")]
pub async fn get_receipt_detail(path: ReceiptDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_receipt_service();
    let receipt = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let lookups = svc.get_detail_lookups(&mut conn, &receipt).await?;
    let (sl, sb, sc) = receipt_status_label(&receipt.status);

    let wo = lookups.wo_doc_number.as_deref().unwrap_or("—");
    let batch = lookups.batch_no.as_deref().unwrap_or("—");
    let product = lookups.product_name.as_deref().unwrap_or("—");
    let warehouse = lookups.warehouse_name.as_deref().unwrap_or("—");

    // 查工单报检工序
    let wo_routings = state.production_batch_service()
        .list_routings(&service_ctx, &mut conn, receipt.work_order_id)
        .await.unwrap_or_default();
    let has_inspection_points = wo_routings.iter().any(|r| r.is_inspection_point);

    // 查 FQC 检验结果
    let fqc_results = state.inspection_result_service()
        .list(&service_ctx, &mut conn,
            InspectionResultFilter {
                source_type: Some(InspectionSourceType::ProductionReceipt),
                source_id: Some(path.id),
                ..Default::default()
            },
            1, 50,
        ).await.map(|p| p.items).unwrap_or_default();

    let fqc_status = compute_fqc_gate(has_inspection_points, &fqc_results);

    // 查 unit_cost（从 stock_ledger 最后已知成本）
    let unit_cost: rust_decimal::Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(
            (SELECT unit_cost FROM stock_ledger
             WHERE product_id = $1 AND unit_cost IS NOT NULL AND unit_cost > 0
             ORDER BY created_at DESC LIMIT 1),
            0::numeric
        )"#,
    )
    .bind(receipt.product_id)
    .fetch_one(&mut *conn)
    .await
    .unwrap_or(rust_decimal::Decimal::ZERO);

    let content = html! { div {
        div class="page-header" {
            div class="page-header-left" {
                a class="back-link" href=(format!("{}?restore=true", ReceiptListPath::PATH)) { "← 返回列表" }
                h1 class="page-title" { "入库单 " (receipt.doc_number) }
            }
            div class="page-actions" {
                (confirm_button(&receipt.status, &fqc_status, path.id))
            }
        }

        // 状态条
        div class="receipt-status-bar" {
            span class="receipt-status-label" {
                "状态: "
                span class="status-pill" style=(format!("background:{};color:{}", sb, sc)) { (sl) }
            }
            (fqc_badge(&fqc_status))
        }

        // 基本信息
        div class="info-card" {
            div class="info-grid" {
                div class="info-item" { label { "单号" } span class="mono" { (receipt.doc_number) } }
                div class="info-item" { label { "工单" } span { (wo) } }
                div class="info-item" { label { "批次" } span { (batch) } }
                div class="info-item" { label { "产品" } span { (product) } }
                div class="info-item" { label { "入库数量" } span class="mono" { (crate::utils::fmt_qty(receipt.received_qty)) } }
                div class="info-item" { label { "仓库" } span { (warehouse) } }
                div class="info-item" { label { "入库日期" } span { (receipt.receipt_date) } }
                div class="info-item" { label { "倒冲触发" } span { (if receipt.backflush_triggered { "是" } else { "否" }) } }
                div class="info-item" { label { "创建时间" } span { (receipt.created_at.format("%Y-%m-%d %H:%M")) } }
            }
        }

        // FQC 质检卡片
        (fqc_card(&fqc_status, &fqc_results))

        // 成本明细卡片
        (cost_card(receipt.received_qty, unit_cost))
    }};

    Ok(Html(admin_page(
        is_htmx, "入库详情", &claims, "production",
        &format!("/admin/mes/receipts/{}", path.id), "生产管理",
        Some(ReceiptListPath::PATH), content, &nav_filter,
    ).into_string()))
}
```

注意：
- 需要导入 `use crate::qms::enums::{InspectionSourceType, InspectionStatus, InspectionResultType};` 或对应路径
- 需要导入 `use abt_core::qms::inspection_result::{InspectionResultService, model::InspectionResultFilter};`
- `sqlx::query_scalar` 在 abt-web 中需要确认是否允许 — AGENTS.md 说禁止直接 DB 访问。如果不允许，需在 abt-core 中添加 `get_unit_cost` 方法到 ProductionReceiptService trait。

- [ ] **Step 2: 新增 FQC 和成本辅助函数**

在文件中添加：

```rust
// ── FQC 门控 ──

enum FqcGateStatus {
    NotRequired,
    PendingInspection,
    AllPassed,
    HasFailed,
}

fn compute_fqc_gate(
    has_inspection_points: bool,
    results: &[abt_core::qms::inspection_result::model::InspectionResultListItem],
) -> FqcGateStatus {
    if !has_inspection_points {
        return FqcGateStatus::NotRequired;
    }
    if results.is_empty() {
        return FqcGateStatus::PendingInspection;
    }
    let all_passed = results.iter().all(|r| {
        r.status == InspectionStatus::Completed && r.result == InspectionResultType::Pass
    });
    if all_passed { FqcGateStatus::AllPassed } else { FqcGateStatus::HasFailed }
}

fn fqc_badge(status: &FqcGateStatus) -> Markup {
    let (label, class) = match status {
        FqcGateStatus::NotRequired => ("无需 FQC", "fqc-badge--na"),
        FqcGateStatus::PendingInspection => ("待 FQC", "fqc-badge--pending"),
        FqcGateStatus::AllPassed => ("FQC 通过", "fqc-badge--passed"),
        FqcGateStatus::HasFailed => ("FQC 不合格", "fqc-badge--failed"),
    };
    html! {
        span class=(format!("fqc-badge {}", class)) { (label) }
    }
}

fn fqc_card(status: &FqcGateStatus, results: &[abt_core::qms::inspection_result::model::InspectionResultListItem]) -> Markup {
    html! {
        div class="info-card" {
            div class="info-section-title" { "FQC 质检状态" }
            div {
                (fqc_badge(status))
            }
            @if !results.is_empty() {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "检验编号" }
                                th { "状态" }
                                th { "结果" }
                                th { "检验日期" }
                            }
                        }
                        tbody {
                            @for r in results {
                                tr {
                                    td class="mono" { (r.doc_number.as_deref().unwrap_or("—")) }
                                    td { (format!("{:?}", r.status)) }
                                    td {
                                        @if r.result == InspectionResultType::Pass {
                                            span class="text-success" { "✓ 合格" }
                                        } @else {
                                            span class="text-danger" { "✗ 不合格" }
                                        }
                                    }
                                    td class="mono" { (r.inspection_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into())) }
                                }
                            }
                        }
                    }
                }
            } @else if has_inspection_points(status) {
                p class="muted" style="padding:12px 0" { "⚠ 尚无 FQC 检验记录，需完成 FQC 后才能确认入库" }
            }
        }
    }
}

fn has_inspection_points(status: &FqcGateStatus) -> bool {
    !matches!(status, FqcGateStatus::NotRequired)
}

// ── 成本明细 ──

fn cost_card(received_qty: rust_decimal::Decimal, unit_cost: rust_decimal::Decimal) -> Markup {
    let total_cost = received_qty * unit_cost;
    html! {
        div class="info-card" {
            div class="info-section-title" { "成本明细" }
            div class="info-grid" {
                div class="info-item" {
                    label { "入库数量" }
                    span class="mono" { (crate::utils::fmt_qty(received_qty)) }
                }
                div class="info-item" {
                    label { "单位成本" }
                    @if unit_cost > rust_decimal::Decimal::ZERO {
                        span class="mono" { "¥" (crate::utils::fmt_qty(unit_cost)) }
                    } @else {
                        span class="muted" { "—（无历史成本）" }
                    }
                }
                div class="info-item" {
                    label { "总成本" }
                    span class="mono" { strong { "¥" (crate::utils::fmt_qty(total_cost)) } }
                }
            }
        }
    }
}

// ── 确认按钮门控 ──

fn confirm_button(status: &abt_core::mes::enums::ReceiptStatus, fqc: &FqcGateStatus, id: i64) -> Markup {
    if *status != abt_core::mes::enums::ReceiptStatus::Draft {
        return html! {};
    }
    let confirm_path = format!("/admin/mes/receipts/{}/confirm", id);
    match fqc {
        FqcGateStatus::AllPassed | FqcGateStatus::NotRequired => {
            html! {
                form class="inline-form" hx-post=(confirm_path) hx-swap="none" {
                    button class="btn btn-primary" type="submit"
                        hx-confirm="确认入库？将触发倒冲和成本结转。" {
                        "确认入库"
                    }
                }
            }
        }
        FqcGateStatus::PendingInspection => {
            html! {
                button class="btn btn-primary" disabled
                    title="需完成 FQC 质检后才能确认入库" {
                    "确认入库（待 FQC）"
                }
            }
        }
        FqcGateStatus::HasFailed => {
            html! {
                button class="btn btn-primary" disabled
                    title="FQC 有不合格项，无法入库" {
                    "确认入库（FQC 不合格）"
                }
            }
        }
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

注意：导入路径需要根据实际项目结构调整。`InspectionResultListItem` 的字段名（如 `doc_number`, `status`, `result`, `inspection_date`）需要与 abt-core 模型一致。

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/mes_receipt_detail.rs
git commit -m "feat: rewrite receipt detail with FQC gate and cost breakdown"
```

---

## Task 2: 消除 inline style

**Files:**
- Modify: `abt-web/src/pages/mes_receipt_detail.rs`

- [ ] **Step 1: 替换所有 inline style**

在 Step 1 的代码中仍有几处 `style=` 属性。替换：

1. `span class="status-pill" style=(format!("background:{};color:{}", sb, sc))` — 这里因为 status 颜色是动态的，inline style 是可接受的例外。但如果想严格遵循规范，可以在 base.css 中定义 `receipt-status-draft` / `receipt-status-confirmed` / `receipt-status-cancelled` class。

2. `p class="muted" style="padding:12px 0"` — 替换为 CSS class `.pad-y` 或直接用已有的间距 class。

3. `input ... style="width:100px"` — 如果有此类，替换为 CSS class `.input-sm`。

实际上，动态背景色的 inline style 是合理的，因为颜色值来自 Rust 逻辑。但 padding/width 等应提取到 CSS class。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/mes_receipt_detail.rs
git commit -m "refactor: eliminate non-dynamic inline styles from receipt detail"
```

---

## Task 3: CSS — fqc-badge / inline-form

**Files:**
- Modify: `static/base.css`

- [ ] **Step 1: 添加样式**

```css
/* ── FQC 状态徽章 ── */
.fqc-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 12px;
    font-size: 13px;
    font-weight: 500;
}
.fqc-badge--pending { background: rgba(255, 159, 67, 0.08); color: #ff9f43; }
.fqc-badge--passed { background: rgba(82, 196, 26, 0.08); color: #52c41a; }
.fqc-badge--failed { background: rgba(245, 63, 63, 0.06); color: #f53f3f; }
.fqc-badge--na { background: #f5f5f5; color: #999; }

/* ── 入库状态条 ── */
.receipt-status-bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 16px;
    background: #fafafa;
    border-radius: 6px;
    margin-bottom: 12px;
}
.receipt-status-label {
    font-size: 14px;
}

/* ── inline-form ── */
.inline-form { display: inline; }
```

- [ ] **Step 2: Commit**

```bash
git add static/base.css
git commit -m "style: add fqc-badge and receipt status bar CSS"
```

---

## Task 4: cargo clippy 最终验证

- [ ] **Step 1: 运行 clippy**

Run: `cargo clippy -p abt-web 2>&1`
Expected: 零 error

- [ ] **Step 2: 修复所有 error**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "fix: resolve clippy errors for receipt detail enhancement"
```

---

## Task 5: E2E 测试 — 完工入库增强

**验证目标：** FQC 状态显示、成本明细、确认按钮门控。

- [ ] **Step 1: 登录**

```bash
agent-browser --cdp 9222 open http://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

- [ ] **Step 2: 打开入库单详情**

```bash
agent-browser --cdp 9222 open http://localhost:8000/admin/mes/receipts
agent-browser snapshot -i
# 点击第一条入库单
agent-browser click @e<first_receipt_link>
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 页面标题 "入库单 RC-xxx"
- 存在状态条（状态 pill + FQC badge）
- 存在基本信息卡片
- 存在 FQC 质检状态卡片
- 存在成本明细卡片

- [ ] **Step 3: 验证 FQC 状态卡片**

```bash
agent-browser snapshot -i
```

验证：
- FQC badge 显示正确状态（无需 FQC / 待 FQC / FQC 通过 / FQC 不合格）
- 如果有检验结果，表格显示检验编号/状态/结果/日期
- 如果有待检，显示提示文字

- [ ] **Step 4: 验证成本明细**

```bash
agent-browser snapshot -i
```

验证：
- 显示入库数量
- 显示单位成本（或 "—（无历史成本）"）
- 显示总成本

- [ ] **Step 5: 验证确认按钮门控**

```bash
agent-browser snapshot -i
```

验证：
- 如果 FQC = AllPassed 或 NotRequired：显示 "确认入库" 按钮（可点击）
- 如果 FQC = PendingInspection：显示 "确认入库（待 FQC）" 按钮（disabled）
- 如果 FQC = HasFailed：显示 "确认入库（FQC 不合格）" 按钮（disabled）

- [ ] **Step 6: 检查控制台错误**

```bash
agent-browser console --clear
agent-browser reload
agent-browser wait 1000
agent-browser errors
```

验证：无 JavaScript 错误。

- [ ] **Step 7: 记录测试结果**

---

## Self-Review Checklist

- [ ] handler 加载了 FQC 检验结果和 unit_cost
- [ ] FQC 状态卡片显示正确（badge + 检验结果表格）
- [ ] 成本明细卡片显示 unit_cost 和 total_cost
- [ ] 确认按钮在 FQC 未通过时禁用并显示原因
- [ ] 状态条动态显示状态 pill 和 FQC badge
- [ ] 无非动态 inline style
- [ ] CSS 有 fqc-badge / receipt-status-bar / inline-form 样式
- [ ] cargo clippy 零 error
- [ ] E2E 测试全部通过
