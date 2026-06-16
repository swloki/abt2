# 领料与退料流程 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 领料单详情页适配 PartiallyIssued 新状态、新增退料流程弹窗、items 表格加工序/批次/成本列。

**Architecture:** 修改 `wms_requisition_detail.rs` 的状态标签和按钮逻辑，新增退料 Modal 和路由 endpoint。列表页加 PartiallyIssued 状态 Tab。

**Tech Stack:** Rust (Maud + HTMX + Hyperscript), abt-core MaterialRequisitionService.return_materials()

---

## File Structure

| 文件 | 职责 | 动作 |
|------|------|------|
| `abt-web/src/pages/wms_requisition_detail.rs` | status_label + action_buttons + items 表格 + return_modal | Modify |
| `abt-web/src/pages/wms_requisition_list.rs` | status tabs 加 PartiallyIssued | Modify |
| `abt-web/src/routes/wms_requisition.rs` | 新增 return endpoint | Modify |
| `static/app.js` | collectReturnItems() | Modify |
| `static/base.css` | status-partial / btn-warning | Modify |

---

## Task 1: status_label 加 PartiallyIssued

**Files:**
- Modify: `abt-web/src/pages/wms_requisition_detail.rs:30-37`

- [ ] **Step 1: 替换 status_label 函数**

```rust
fn status_label(s: RequisitionStatus) -> (&'static str, &'static str) {
    match s {
        RequisitionStatus::Draft => ("草稿", "status-draft"),
        RequisitionStatus::Confirmed => ("已确认", "status-confirmed"),
        RequisitionStatus::PartiallyIssued => ("部分发料", "status-partial"),
        RequisitionStatus::Issued => ("已发料", "status-completed"),
        RequisitionStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/wms_requisition_detail.rs
git commit -m "feat: add PartiallyIssued status label"
```

---

## Task 2: action_buttons 加 PartiallyIssued 分支 + 退料按钮

**Files:**
- Modify: `abt-web/src/pages/wms_requisition_detail.rs:285-329`

- [ ] **Step 1: 替换 requisition_action_buttons 函数**

```rust
fn requisition_action_buttons(status: RequisitionStatus, detail_path: &str) -> Markup {
    match status {
        RequisitionStatus::Draft => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此领料单吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"confirm"}"#
                    hx-confirm="确定要确认此领料单吗？"
                    hx-redirect=(detail_path) {
                    (icon::check_circle_icon("w-4 h-4"))
                    "确认"
                }
            }
        }
        RequisitionStatus::Confirmed => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此领料单吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"issue"}"#
                    hx-confirm="确定要确认发料吗？实发数量将按需求数量自动填写。"
                    hx-redirect=(detail_path) {
                    (icon::bolt_icon("w-4 h-4"))
                    "确认发料"
                }
            }
        }
        RequisitionStatus::PartiallyIssued => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此领料单吗？已发出的物料不会回滚。"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"issue"}"#
                    hx-confirm="继续发料？将补发剩余物料。"
                    hx-redirect=(detail_path) {
                    (icon::bolt_icon("w-4 h-4"))
                    "继续发料"
                }
                button class="btn btn-warning"
                    _="on click add .is-open to #return-modal" {
                    (icon::undo_icon("w-4 h-4"))
                    "退料"
                }
            }
        }
        RequisitionStatus::Issued => {
            html! {
                button class="btn btn-warning"
                    _="on click add .is-open to #return-modal" {
                    (icon::undo_icon("w-4 h-4"))
                    "退料"
                }
            }
        }
        _ => html! {},
    }
}
```

注意：确认 `icon` 模块中有 `undo_icon` 函数。如果没有，用 `icon::arrow_left_icon` 或其他相近 icon 代替。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/wms_requisition_detail.rs
git commit -m "feat: add PartiallyIssued action buttons and return button"
```

---

## Task 3: items 表格新增列

**Files:**
- Modify: `abt-web/src/pages/wms_requisition_detail.rs:244-280`

- [ ] **Step 1: 替换 items 表格区域**

在 `requisition_detail_page` 函数中，将行项明细表格（约 244-280 行）替换为：

```rust
            // ── 行项明细 ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品" }
                                th class="num-right" { "需求数量" }
                                th class="num-right" { "实领数量" }
                                th class="num-right" { "差异量" }
                                th { "工序" }
                                th { "批次" }
                                th class="num-right" { "单位成本" }
                                th class="num-right" { "金额" }
                                th { "储位" }
                            }
                        }
                        tbody {
                            @for (i, item) in items.iter().enumerate() {
                                @let (variance_text, variance_class) = variance_color_class(item.variance_qty);
                                @let cost_amount = item.issued_qty * item.unit_cost.unwrap_or(rust_decimal::Decimal::ZERO);
                                tr {
                                    td class="mono" { (i + 1) }
                                    td { (product_names.get(&item.product_id).map(|n| n.as_str()).unwrap_or("—")) }
                                    td class="num-right" { (format!("{:.2}", item.requested_qty)) }
                                    td class="num-right" { (format!("{:.2}", item.issued_qty)) }
                                    td class=(format!("num-right {}", variance_class)) { (variance_text) }
                                    td class="mono" {
                                        @if let Some(op_id) = item.operation_id {
                                            "#" (op_id)
                                        } @else { "—" }
                                    }
                                    td class="mono" {
                                        @if let Some(b_id) = item.batch_id {
                                            "#" (b_id)
                                        } @else { "—" }
                                    }
                                    td class="num-right" {
                                        @if let Some(uc) = item.unit_cost {
                                            "¥" (format!("{:.2}", uc))
                                        } @else { "—" }
                                    }
                                    td class="num-right" {
                                        @if cost_amount > rust_decimal::Decimal::ZERO {
                                            "¥" (format!("{:.2}", cost_amount))
                                        } @else { "—" }
                                    }
                                    td { (item.bin_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into())) }
                                }
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="10" class="empty-row" { "暂无领料明细" }
                                }
                            }
                        }
                    }
                }
                // 成本汇总
                @let total_cost: rust_decimal::Decimal = items.iter()
                    .map(|i| i.issued_qty * i.unit_cost.unwrap_or(rust_decimal::Decimal::ZERO))
                    .sum();
                @if total_cost > rust_decimal::Decimal::ZERO {
                    div class="amount-summary" {
                        div class="amount-row" {
                            span { "发料总金额" }
                            span class="mono amount-value" { "¥" (format!("{:.2}", total_cost)) }
                        }
                    }
                }
            }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

注意：`MaterialReqItem` 必须有 `unit_cost: Option<Decimal>` 字段。已在核心改动文档③中添加。

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/wms_requisition_detail.rs
git commit -m "feat: add operation/batch/cost columns to requisition items"
```

---

## Task 4: 退料 Modal

**Files:**
- Modify: `abt-web/src/pages/wms_requisition_detail.rs`

- [ ] **Step 1: 新增 return_modal 函数**

在 `requisition_detail_page` 函数的 `html!` 块末尾（约第 281 行 `}` 之前）添加调用：

```rust
            // 退料弹窗（仅 Issued / PartiallyIssued 状态显示）
            @if matches!(requisition.status, RequisitionStatus::Issued | RequisitionStatus::PartiallyIssued) {
                (return_modal(items, detail_path, product_names))
            }
```

然后在文件中新增 `return_modal` 函数：

```rust
fn return_modal(
    items: &[abt_core::wms::material_requisition::model::MaterialReqItem],
    detail_path: &str,
    product_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    let return_path = format!("{}/return", detail_path);
    let issued_items: Vec<_> = items.iter().filter(|i| i.issued_qty > rust_decimal::Decimal::ZERO).collect();

    html! {
        div class="modal-overlay" id="return-modal" {
            div class="modal modal-lg" {
                div class="modal-head" {
                    h2 { "退料" }
                    button _="on click remove .is-open from #return-modal" { "×" }
                }
                form hx-post=(return_path)
                    hx-redirect=(detail_path) {
                    div class="modal-body" {
                        p class="modal-desc" { "选择要退料的物料，填写退料数量（不能超过已发量）：" }

                        @if issued_items.is_empty() {
                            p class="empty-row" { "无已发料物料可退" }
                        } @else {
                            table class="data-table return-table" {
                                thead {
                                    tr {
                                        th style="width:40px" { input type="checkbox" _="on click toggle @checked on .return-check" {}; }
                                        th { "产品" }
                                        th class="num-right" { "已发量" }
                                        th class="num-right" { "退料量" }
                                    }
                                }
                                tbody {
                                    @for item in &issued_items {
                                        tr {
                                            td {
                                                input type="checkbox" class="return-check"
                                                      name=(format!("check_{}", item.id))
                                                      value="true";
                                            }
                                            td { (product_names.get(&item.product_id).map(|n| n.as_str()).unwrap_or("—")) }
                                            td class="mono num-right" { (format!("{:.2}", item.issued_qty)) }
                                            td {
                                                input type="number" class="form-input return-qty"
                                                      name=(format!("return_qty_{}", item.id))
                                                      max=(format!("{:.3}", item.issued_qty))
                                                      value="0"
                                                      step="0.001"
                                                      style="width:100px";
                                            }
                                            input type="hidden"
                                                  name=(format!("product_id_{}", item.id))
                                                  value=(item.product_id);
                                            input type="hidden"
                                                  name=(format!("original_qty_{}", item.id))
                                                  value=(format!("{:.3}", item.issued_qty));
                                        }
                                    }
                                }
                            }
                            div class="form-grid" {
                                div class="form-field span-2" {
                                    label { "退料原因 *" }
                                    input type="text" class="form-input" name="reason" required;
                                }
                            }
                            input type="hidden" name="items_json" id="return-items-json";
                        }
                    }
                    div class="modal-foot" {
                        button class="btn btn-default" type="button"
                            _="on click remove .is-open from #return-modal" {
                            "取消"
                        }
                        button class="btn btn-warning" type="submit"
                            _="on click call collectReturnItems() then put it into #return-items-json then add .hidden to me"
                            hx-confirm="确认退料？退料物料将入库并扣减已发数量。" {
                            "确认退料"
                        }
                    }
                }
            }
        }
    }
}
```

注意：`<th style="width:40px">` 是 `<col>` 类似场景的例外，但按规范应避免。如果需要，可以用 CSS class `.return-check-col`。

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/wms_requisition_detail.rs
git commit -m "feat: add return material modal"
```

---

## Task 5: 退料路由 endpoint

**Files:**
- Modify: `abt-web/src/routes/wms_requisition.rs`

- [ ] **Step 1: 添加退料 TypedPath**

在 `RequisitionDetailPath` 之后添加：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/requisitions/{id}/return")]
pub struct RequisitionReturnPath {
    pub id: i64,
}
```

- [ ] **Step 2: 注册路由**

在 `router()` 函数中添加：

```rust
        .route(
            RequisitionReturnPath::PATH,
            post(wms_requisition_detail::post_return),
        )
```

注意：需要在文件顶部 `use axum::routing::{get, post};` 将 `get` 改为 `get, post`。

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/routes/wms_requisition.rs
git commit -m "feat: add return material route endpoint"
```

---

## Task 6: 退料 Handler

**Files:**
- Modify: `abt-web/src/pages/wms_requisition_detail.rs`

- [ ] **Step 1: 新增 post_return handler**

在文件中添加（需要 `use axum::response::IntoResponse;`）：

```rust
#[derive(Debug, serde::Deserialize)]
pub struct ReturnForm {
    pub items_json: String,
    pub reason: String,
}

#[require_permission("INVENTORY", "update")]
pub async fn post_return(
    path: crate::routes::wms_requisition::RequisitionReturnPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReturnForm>,
) -> crate::errors::Result<axum::response::Response> {
    use abt_core::wms::material_requisition::model::{ReturnMaterialReq, ReturnItemReq};

    let RequestContext { state, service_ctx, .. } = ctx;

    let items: Vec<ReturnItemReq> = serde_json::from_str(&form.items_json)
        .map_err(|e| abt_core::shared::types::DomainError::validation(format!("无效退料数据: {e}")))?;

    if items.is_empty() {
        return Err(abt_core::shared::types::DomainError::validation("请至少选择一项退料物料").into());
    }

    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    state.material_requisition_service()
        .return_materials(&service_ctx, &mut tx, path.id, ReturnMaterialReq {
            items,
            reason: form.reason,
        }).await?;

    tx.commit().await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &format!("/admin/wms/requisitions/{}", path.id))
        .body(axum::body::Body::empty()).unwrap())
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 3: Commit**

```bash
git add abt-web/src/pages/wms_requisition_detail.rs
git commit -m "feat: add return material handler"
```

---

## Task 7: collectReturnItems() JS 函数

**Files:**
- Modify: `static/app.js`

- [ ] **Step 1: 在 app.js 末尾添加**

```javascript
/**
 * 收集退料弹窗中勾选的物料行
 * @returns {string} JSON 字符串
 */
function collectReturnItems() {
    const checks = document.querySelectorAll('.return-check:checked');
    const items = Array.from(checks).map(check => {
        const idMatch = check.name.match(/check_(\d+)/);
        if (!idMatch) return null;
        const itemId = parseInt(idMatch[1]);
        const row = check.closest('tr');
        const qtyInput = row.querySelector(`input[name="return_qty_${itemId}"]`);
        const pidInput = row.querySelector(`input[name="product_id_${itemId}"]`);
        const returnQty = parseFloat(qtyInput.value) || 0;
        if (returnQty <= 0) return null;
        return {
            item_id: itemId,
            product_id: parseInt(pidInput.value),
            return_qty: returnQty,
        };
    }).filter(i => i !== null);
    return JSON.stringify(items);
}
```

- [ ] **Step 2: Commit**

```bash
git add static/app.js
git commit -m "feat: add collectReturnItems JS function"
```

---

## Task 8: 列表页 status tabs 加 PartiallyIssued

**Files:**
- Modify: `abt-web/src/pages/wms_requisition_list.rs`

- [ ] **Step 1: 找到状态 Tab 定义**

搜索 `wms_requisition_list.rs` 中的 status filter 或 status tabs 定义。

在状态选项中添加 PartiallyIssued：

```rust
// 在状态 Tab 或 filter select 中添加：
("5", "部分发料"),
```

具体位置取决于现有代码结构 — 通常在 status tabs 构建函数或 filter 解析函数中。

- [ ] **Step 2: 更新 filter 解析**

如果有 `parse_status_filter` 或类似函数，添加：

```rust
Some("5") => Some(RequisitionStatus::PartiallyIssued),
```

- [ ] **Step 3: 验证编译**

Run: `cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 4: Commit**

```bash
git add abt-web/src/pages/wms_requisition_list.rs
git commit -m "feat: add PartiallyIssued to requisition list status tabs"
```

---

## Task 9: CSS — status-partial / btn-warning

**Files:**
- Modify: `static/base.css`

- [ ] **Step 1: 在 base.css 中添加样式**

```css
/* ── PartiallyIssued 状态 ── */
.status-partial {
    background: rgba(255, 159, 67, 0.08);
    color: #ff9f43;
}

/* ── 退料按钮 ── */
.btn-warning {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 6px 12px;
    border-radius: var(--radius, 4px);
    font-size: var(--text-sm, 14px);
    font-weight: 500;
    background: rgba(255, 159, 67, 0.1);
    color: #ff9f43;
    border: 1px solid rgba(255, 159, 67, 0.2);
    cursor: pointer;
    transition: all 0.2s;
}
.btn-warning:hover {
    background: rgba(255, 159, 67, 0.2);
}
.btn-warning:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

/* ── 成本汇总 ── */
.amount-summary {
    padding: 12px 16px;
    border-top: 1px solid var(--border, #e5e7eb);
    display: flex;
    justify-content: flex-end;
}
.amount-row {
    display: flex;
    gap: 8px;
    align-items: center;
}
.amount-value {
    font-size: 16px;
    font-weight: 600;
    color: var(--primary, #165dff);
}

/* ── 退料表格复选框列 ── */
.return-table th:first-child,
.return-table td:first-child {
    text-align: center;
}
```

- [ ] **Step 2: Commit**

```bash
git add static/base.css
git commit -m "style: add status-partial, btn-warning, amount-summary CSS"
```

---

## Task 10: cargo clippy 最终验证

- [ ] **Step 1: 运行完整 clippy**

Run: `cargo clippy -p abt-web 2>&1`
Expected: 零 error

- [ ] **Step 2: 修复所有 error**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "fix: resolve clippy errors for requisition return flow"
```

---

## Task 11: E2E 测试 — 领料与退料流程

**验证目标：** 部分发料状态显示、退料弹窗交互、成本列展示。

- [ ] **Step 1: 登录**

```bash
agent-browser --cdp 9222 open http://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

- [ ] **Step 2: 测试列表页 PartiallyIssued Tab**

```bash
agent-browser --cdp 9222 open http://localhost:8000/admin/wms/requisitions
agent-browser snapshot -i
```

验证：
- 状态 Tab / 筛选器包含 "部分发料" 选项
- 切换到 "部分发料" 后只显示该状态的领料单

- [ ] **Step 3: 打开领料单详情（Issued 或 PartiallyIssued 状态）**

```bash
# 从列表点击一条已发料的领料单
agent-browser click @e<issued_requisition_link>
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 状态标签显示正确（"部分发料" 或 "已发料"）
- items 表格包含工序列、批次列、单位成本列、金额列
- 存在成本汇总区域（如有成本数据）
- 存在 "退料" 按钮

- [ ] **Step 4: 测试退料弹窗**

```bash
agent-browser click @e<return_button>
agent-browser wait 500
agent-browser snapshot -i
```

验证：
- 弹窗显示，标题 "退料"
- 表格列出已发料物料
- 每行有复选框和退料数量输入框
- 存在 "退料原因" 输入框
- 存在 "取消" 和 "确认退料" 按钮

- [ ] **Step 5: 测试退料操作**

```bash
# 勾选第一行
agent-browser click @e<first_return_check>
# 填入退料数量
agent-browser fill @e<first_return_qty> "5"
# 填入原因
agent-browser fill @e<reason_input> "E2E测试退料"
# 提交
agent-browser click @e<confirm_return_button>
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 页面刷新后领料单状态可能更新
- 审计日志中有退料记录

- [ ] **Step 6: 测试部分发料状态的按钮**

如果有 PartiallyIssued 状态的领料单：

```bash
agent-browser --cdp 9222 open http://localhost:8000/admin/wms/requisitions
agent-browser snapshot -i
# 筛选部分发料
agent-browser select @e<status_filter> "5"
agent-browser wait 500
agent-browser snapshot -i
# 点击第一条
agent-browser click @e<first_requisition_link>
agent-browser wait 1000
agent-browser snapshot -i
```

验证：
- 显示 "继续发料" 按钮
- 显示 "退料" 按钮
- 显示 "取消" 按钮

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

- [ ] status_label 包含 PartiallyIssued → ("部分发料", "status-partial")
- [ ] action_buttons 在 PartiallyIssued 时显示"继续发料" + "退料"
- [ ] action_buttons 在 Issued 时显示"退料"
- [ ] items 表格有工序列、批次列、单位成本列、金额列
- [ ] 成本汇总正确计算
- [ ] 退料弹窗可选择物料行并填写退料数量
- [ ] collectReturnItems() 正确收集数据
- [ ] 列表页有 PartiallyIssued Tab
- [ ] CSS 有 status-partial / btn-warning / amount-summary 样式
- [ ] cargo clippy 零 error
- [ ] E2E 测试全部通过
