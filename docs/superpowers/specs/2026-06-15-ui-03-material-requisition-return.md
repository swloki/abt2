# UI-03: 领料与退料流程

> 核心改动牵涉：文档③（领料完善）— PartiallyIssued 状态、退料方法、operation_id/batch_id、unit_cost
> Odoo 参考：`stock.move`（消耗/退料）+ `mrp.production` Components Tab

## 1. 目标

领料单页面需适配核心改动后的新状态流转（Draft → Confirmed → PartiallyIssued → Issued）、新增退料流程、展示工序/批次关联和成本数据。

## 2. 当前状态

### 已有

- `wms_requisition_list.rs` — 列表页，含状态 Tab
- `wms_requisition_detail.rs` — 详情页，含 workflow_steps + action_buttons
- `wms_requisition_create.rs` — 创建页（手工创建 + 工单关联创建）
- 状态：Draft / Confirmed / Issued / Cancelled

### 缺失

| 差距 | 位置 | 说明 |
|------|------|------|
| PartiallyIssued 状态 | `status_label` + list status tabs | 新增枚举值=5，label="部分发料" |
| 继续发料按钮 | `requisition_action_buttons` | PartiallyIssued 状态下可继续发料 |
| 退料按钮 + 弹窗 | `requisition_action_buttons` | Issued/PartiallyIssued 状态下可退料 |
| operation_id 列 | items 表格 | 关联工序名称 |
| batch_id 列 | items 表格 | 关联批次编号 |
| unit_cost 列 | items 表格 | 单位成本 |
| cost_amount 列 | items 表格 | 行金额 = issued_qty × unit_cost |
| 成本汇总 | 详情页底部 | 总发料金额 |

## 3. Odoo 参考

### Odoo `stock.move` — 消耗对比

```
Components Tab（mrp.production 表单内）:
┌──────────────────────────────────────────────────────┐
│ Product    │ To Consume │ Reserved │ Consumed │ Unit │
│ 原料A      │ 50 kg      │ 50 kg    │ 48 kg    │ kg   │
│ 原料B      │ 30 kg      │ 30 kg    │ 30 kg    │ kg   │
│ 螺丝M4     │ 200 pc     │ 0 pc     │ 0 pc     │ pc   │
├──────────────────────────────────────────────────────┤
│ Reserved: ✓ Available / ⚠ Partially / ✗ Not Available │
│ [Check Availability] [Produce]                        │
└──────────────────────────────────────────────────────┘
```

**关键 Odoo 模式**：
1. **To Consume vs Consumed** — 计划消耗 vs 实际消耗对比
2. **Reserved** — 预留状态（Available/Partially/None）
3. **Return** — 通过 `stock.move` 的 `origin_returned_move_id` 创建反向 move

### 我们的适配

退料功能通过新 endpoint + modal 表单实现（非 Odoo 的向导式弹窗）。成本计算沿用核心层已实现的 `unit_cost` 查询（stock_ledger 最后已知成本）。

## 4. 修改设计

### 4.1 状态标签扩展

```rust
// wms_requisition_detail.rs — status_label 扩展
fn status_label(s: RequisitionStatus) -> (&'static str, &'static str) {
    match s {
        RequisitionStatus::Draft => ("草稿", "status-draft"),
        RequisitionStatus::Confirmed => ("已确认", "status-confirmed"),
        RequisitionStatus::PartiallyIssued => ("部分发料", "status-partial"),  // ← 新增
        RequisitionStatus::Issued => ("已发料", "status-completed"),
        RequisitionStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// wms_requisition_list.rs — status tabs 扩展
fn status_tabs(active: &str) -> Vec<(&str, &str)> {
    vec![
        ("", "全部"),
        ("1", "草稿"),
        ("2", "已确认"),
        ("5", "部分发料"),  // ← 新增
        ("3", "已发料"),
        ("4", "已取消"),
    ]
}
```

### 4.2 状态流转图更新

```
Draft ──confirm──→ Confirmed ──issue──→ (全部发完?) ──Yes──→ Issued
                       │                    │
                       │                    No
                       │                    ↓
                       │               PartiallyIssued ──issue──→ (同上)
                       │                    │
                       └──cancel──→ Cancelled ←──cancel──┘
```

`workflow_steps` 函数需新增 PartiallyIssued 节点：

```rust
fn workflow_steps(status: RequisitionStatus) -> Markup {
    html! {
        div class="workflow-steps" {
            // 草稿 → 已确认 → 部分发料 → 已发料
            (wf_step("草稿", status == Draft || is_after(status, Draft)))
            (wf_step("已确认", status >= Confirmed))
            @if status == PartiallyIssued || has_partial_history {
                (wf_step_active("部分发料"))
            }
            (wf_step("已发料", status == Issued))
        }
    }
}
```

### 4.3 Action 按钮扩展

```rust
fn requisition_action_buttons(status: RequisitionStatus, has_returns: bool, detail_path: &str) -> Markup {
    match status {
        Draft => { /* 取消 + 确认 — 不变 */ }
        Confirmed => {
            html! {
                // 取消 — 不变
                // 确认发料 — 不变
            }
        }
        PartiallyIssued => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此领料单吗？已发出的物料不会回滚。"
                    hx-redirect=(detail_path) {
                    "取消"
                }
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"issue"}"#
                    hx-confirm="继续发料？将补发剩余物料。" 
                    hx-redirect=(detail_path) {
                    "继续发料"
                }
                (return_button(detail_path))  // ← 退料按钮
            }
        }
        Issued => {
            html! {
                (return_button(detail_path))  // ← 退料按钮
            }
        }
        _ => html! {},
    }
}

fn return_button(detail_path: &str) -> Markup {
    html! {
        button class="btn btn-warning"
            _="on click add .is-open to #return-modal" {
            "退料"
        }
    }
}
```

### 4.4 退料 Modal

```
┌── 退料 ────────────────────────────────────────────────┐
│                                                  [×]   │
│ 选择要退料的物料，填写退料数量：                        │
│                                                        │
│ ┌────────────────────────────────────────────────┐     │
│ │ ✓ │ 产品       │ 已发量 │ 退料量  │ 退料仓库    │     │
│ │ ☑ │ 原料A      │ 48    │ [48  ] │ [主仓库 ▾] │     │
│ │ ☐ │ 原料B      │ 30    │ [0   ] │ [—      ]  │     │
│ │ ☑ │ 螺丝M4     │ 200   │ [50  ] │ [主仓库 ▾] │     │
│ └────────────────────────────────────────────────┘     │
│                                                        │
│ 退料原因 *    [_______________________________]        │
│ 备注          [_______________________________]        │
│                                                        │
│                          [取消]  [确认退料]             │
└────────────────────────────────────────────────────────┘
```

**实现**：

```rust
fn return_modal(items: &[MaterialReqItem], product_names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="modal-overlay" id="return-modal" {
            div class="modal modal-lg" {
                div class="modal-head" {
                    h3 { "退料" }
                    button _="on click remove .is-open from #return-modal" { "×" }
                }
                form class="modal-body"
                    hx-post=(format!("{}/return", detail_path))
                    hx-target="#return-modal"
                    hx-swap="outerHTML"
                    hx-redirect=(detail_path) {
                    p class="modal-desc" { "选择要退料的物料，填写退料数量（不能超过已发量）：" }
                    table class="data-table return-table" {
                        thead {
                            tr {
                                th { input type="checkbox" _="on click toggle @checked on .return-check" {}; }
                                th { "产品" }
                                th class="num-right" { "已发量" }
                                th class="num-right" { "退料量" }
                                th { "退料仓库" }
                            }
                        }
                        tbody {
                            @for item in items.iter().filter(|i| i.issued_qty > Decimal::ZERO) {
                                tr {
                                    td {
                                        input type="checkbox" class="return-check"
                                              name=(format!("items[{}].selected", item.id))
                                              value="true";
                                    }
                                    td { (product_names.get(&item.product_id).cloned().unwrap_or_default()) }
                                    td class="mono num-right" { (fmt_qty(item.issued_qty)) }
                                    td {
                                        input type="number" class="form-input return-qty"
                                              name=(format!("items[{}].return_qty", item.id))
                                              max=(item.issued_qty.to_string())
                                              value="0"
                                              step="0.001";
                                    }
                                    td {
                                        select class="form-select" name=(format!("items[{}].warehouse_id", item.id)) {
                                            option value="1" { "主仓库" }
                                            // 动态加载仓库列表
                                        }
                                    }
                                    // hidden 字段
                                    input type="hidden"
                                          name=(format!("items[{}].product_id", item.id))
                                          value=(item.product_id);
                                    input type="hidden"
                                          name=(format!("items[{}].original_qty", item.id))
                                          value=(item.issued_qty);
                                }
                            }
                        }
                    }
                    div class="form-grid" {
                        div class="form-field span-2" {
                            label { "退料原因 *" }
                            input type="text" class="form-input" name="reason" required;
                        }
                        div class="form-field span-2" {
                            label { "备注" }
                            textarea class="form-input" name="remark" {};
                        }
                    }
                    // JS 收集勾选行
                    input type="hidden" name="items_json" id="return-items-json"
                           _="on click call collectReturnItems() then put it into my value";
                }
                div class="modal-foot" {
                    button class="btn btn-default"
                        _="on click remove .is-open from #return-modal" {
                        "取消"
                    }
                    button class="btn btn-warning" type="submit"
                        formaction=(format!("{}/return", detail_path))
                        hx-confirm="确认退料？退料物料将入库并扣减已发数量。" {
                        "确认退料"
                    }
                }
            }
        }
    }
}
```

**退料 JS** (`static/app.js` 新增)：

```javascript
function collectReturnItems() {
    const checks = document.querySelectorAll('.return-check:checked');
    return Array.from(checks).map(check => {
        const row = check.closest('tr');
        const id = check.name.match(/items\[(\d+)\]/)[1];
        const qtyInput = row.querySelector(`input[name="items[${id}].return_qty"]`);
        const whInput = row.querySelector(`select[name="items[${id}].warehouse_id"]`);
        const pidInput = row.querySelector(`input[name="items[${id}].product_id"]`);
        return {
            item_id: parseInt(id),
            product_id: parseInt(pidInput.value),
            return_qty: parseFloat(qtyInput.value) || 0,
            warehouse_id: parseInt(whInput.value),
        };
    }).filter(i => i.return_qty > 0);
}
```

### 4.5 Items 表格增强

当前 items 表格列：行号 / 产品 / 需求量 / 实发量 / 差异

**新增列**：

| 新列 | 数据 | 说明 |
|------|------|------|
| 工序 | `item.operation_id` → 名称 | 工单领料时填充 |
| 批次 | `item.batch_id` → 编号 | 批次领料时填充 |
| 单位成本 | `item.unit_cost` | 从 stock_ledger 查询 |
| 金额 | `item.issued_qty × item.unit_cost` | 行金额 |

**替换后**：

```
行号 │ 产品     │ 需求量 │ 实发量 │ 差异  │ 工序 │ 批次  │ 单位成本 │ 金额
 1   │ 原料A    │ 50    │ 48    │ -2   │ 注塑 │ B001 │ ¥10.00  │ ¥480.00
 2   │ 原料B    │ 30    │ 30    │ 0    │ 注塑 │ B001 │ ¥25.00  │ ¥750.00
 3   │ 螺丝M4   │ 200   │ 200   │ 0    │ 组装 │ B001 │ ¥0.50   │ ¥100.00
──────────────────────────────────────────────────────────────────────
                                             总计: ¥1,330.00
```

### 4.6 成本汇总

在 items 表格下方新增汇总区：

```rust
fn cost_summary(items: &[MaterialReqItem]) -> Markup {
    let total: Decimal = items.iter()
        .map(|i| i.issued_qty * i.unit_cost.unwrap_or(Decimal::ZERO))
        .sum();
    html! {
        div class="amount-summary" {
            div class="amount-row" {
                span { "发料总金额" }
                span class="mono amount-value" { (fmt_money(total)) }
            }
        }
    }
}
```

## 5. 新增路由

### POST `/admin/wms/requisitions/:id/return`

```rust
// routes/wms_requisition.rs 新增
#[derive(TypedPath, Deserialize)]
#[typed_path("/admin/wms/requisitions/{requisition_id}/return")]
pub struct RequisitionReturnPath {
    pub requisition_id: i64,
}

// handler
#[require_permission("INVENTORY", "update")]
pub async fn post_return(
    path: RequisitionReturnPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReturnForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let items: Vec<ReturnItemReq> = serde_json::from_str(&form.items_json)?;
    let mut tx = state.pool.begin().await?;
    state.material_requisition_service()
        .return_materials(&service_ctx, &mut tx, path.requisition_id, ReturnMaterialReq {
            items,
            reason: form.reason,
            remark: form.remark.unwrap_or_default(),
        }).await?;
    tx.commit().await?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &format!("/admin/wms/requisitions/{}", path.requisition_id))
        .body(axum::body::Body::empty()).unwrap())
}

#[derive(serde::Deserialize)]
pub struct ReturnForm {
    pub items_json: String,
    pub reason: String,
    pub remark: Option<String>,
}
```

## 6. 列表页状态 Tab 更新

`wms_requisition_list.rs` 的 status tabs 需加上 PartiallyIssued：

```rust
// status filter 解析
fn parse_status_filter(s: &Option<String>) -> Option<RequisitionStatus> {
    match s.as_deref() {
        Some("1") => Some(RequisitionStatus::Draft),
        Some("2") => Some(RequisitionStatus::Confirmed),
        Some("5") => Some(RequisitionStatus::PartiallyIssued),  // ← 新增
        Some("3") => Some(RequisitionStatus::Issued),
        Some("4") => Some(RequisitionStatus::Cancelled),
        _ => None,
    }
}
```

## 7. CSS 新增

```css
/* status-partial — 部分发料状态 */
.status-partial {
    background: rgba(255, 159, 67, 0.08);
    color: var(--warning);
}

/* btn-warning — 退料按钮 */
.btn-warning {
    background: rgba(255, 159, 67, 0.1);
    color: var(--warning);
    border: 1px solid rgba(255, 159, 67, 0.2);
}
.btn-warning:hover {
    background: rgba(255, 159, 67, 0.2);
}

/* return-table checkbox 列 */
.return-table th:first-child,
.return-table td:first-child {
    width: 40px;
    text-align: center;
}
```

## 8. 实现步骤

1. `wms_requisition_detail.rs`:
   - `status_label` — 加 PartiallyIssued
   - `workflow_steps` — 加部分发料节点
   - `requisition_action_buttons` — 加 PartiallyIssued 分支 + return_button
   - `return_modal` — 新增退料弹窗
   - items 表格 — 加工序/批次/成本列
   - `cost_summary` — 新增成本汇总
2. `wms_requisition_list.rs`:
   - status tabs — 加部分发料
   - `parse_status_filter` — 加 value=5
3. `routes/wms_requisition.rs`:
   - 新增 `RequisitionReturnPath` + `post_return` handler
4. `static/app.js`:
   - 新增 `collectReturnItems()` 函数
5. `base.css`:
   - 加 `.status-partial` / `.btn-warning` 样式

## 9. 验收标准

- [ ] 部分发料状态在列表 Tab 和详情 pill 正确显示
- [ ] PartiallyIssued 状态下"继续发料"按钮可用
- [ ] 退料弹窗可选择物料行并填写退料数量
- [ ] 退料提交后：入库数量正确、issued_qty 扣减、审计日志记录
- [ ] items 表格显示工序/批次/单位成本/金额
- [ ] 成本汇总正确
- [ ] cargo clippy 零错误
