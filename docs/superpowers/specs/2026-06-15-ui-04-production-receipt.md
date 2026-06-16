# UI-04: 完工入库增强

> 核心改动牵涉：文档⑤（完工入库完善）— FQC 门控、成本计算、倒冲事务、条件性 PlanItem 完成
> Odoo 参考：`mrp.production` "Produce" + 质量检查门控

## 1. 目标

完工入库详情页需展示 FQC 质检状态、成本明细、倒冲结果，并在确认按钮上实现 FQC 门控。

## 2. 当前状态

### 已有

`mes_receipt_detail.rs` — 极简页面：
- 头部：返回 + "确认入库" 按钮（Draft 状态显示）
- 单个 `info-card`：10 个 info-item（单号/工单/批次/产品/数量/仓库/日期/状态/倒冲触发/创建时间）
- 无 Tab 结构
- 无 FQC 展示
- 无成本明细

### 问题

| 差距 | 严重度 | 说明 |
|------|--------|------|
| 无 FQC 质检状态 | P0 | 用户看不到是否需 FQC、FQC 是否通过 |
| 确认按钮无 FQC 门控 | P0 | Draft 状态无条件显示，点击后由后端报错 |
| 无成本明细 | P0 | 仅有数量，无 unit_cost / total_cost |
| 倒冲只显示"是/否" | P1 | 无倒冲详情（消耗了哪些物料） |
| 使用 inline `style` | P1 | 违反 CLAUDE.md 禁止内联样式规则 |
| 无关联工单/批次跳转 | P2 | 工单/批次只显示编号，无链接 |

## 3. Odoo 参考

### Odoo Produce Wizard (`mrp.production` → "Produce" 按钮)

```
┌── Produce ────────────────────────────────────────┐
│ Manufacturing Order: MO/2024/001                   │
│ Product: 电源板A                                   │
│                                                    │
│ ── Quality Checks ──                               │
│ ⚠ FQC inspection required before producing         │
│   [Go to Inspection]                               │
│                                                    │
│ ── Quantity ──                                     │
│ To Produce: 100    Producing: [100 ]               │
│                                                    │
│ ── Cost Preview ──                                 │
│ Unit Cost: ¥15.00    Total: ¥1,500.00              │
│                                                    │
│                          [Cancel] [Confirm]        │
└────────────────────────────────────────────────────┘
```

**关键 Odoo 模式**：
1. **Quality Check Gate** — 有未通过质检时阻止生产
2. **Cost Preview** — 确认前展示预估成本
3. **Lot/Serial** — 批次/序列号追踪

### 我们的适配

不做向导式弹窗，改为详情页内信息卡片展示 + 条件性确认按钮。FQC 状态从 inspection_results 查询。

## 4. 修改设计

### 4.1 页面整体重构 — 从单卡片改为 Tab 结构

```
┌─────────────────────────────────────────────────────────────┐
│ ← 返回列表    入库单 RC-2024-001                              │
│                                                              │
│ ┌── 状态条 ──────────────────────────────────────────────┐  │
│ │ 状态: ● 草稿          FQC: ⏳ 待检                     │  │
│ │ [确认入库] (FQC 未通过时禁用)                           │  │
│ └────────────────────────────────────────────────────────┘  │
│                                                              │
│ [基本信息]  [FQC 质检]  [成本明细]  [倒冲详情]               │
│                                                              │
│ ── 基本信息 ──                                               │
│  ┌── 入库信息 ──────────────┐  ┌── 关联 ─────────────────┐  │
│  │ 单号: RC-2024-001        │  │ 工单: MO-2024-001 →    │  │
│  │ 入库数量: 100             │  │ 批次: B001 →           │  │
│  │ 产品: 电源板A             │  │ 仓库: 主仓库           │  │
│  │ 入库日期: 2024-06-15      │  │                        │  │
│  │ 倒冲触发: 是              │  │                        │  │
│  │ 创建时间: ...             │  │                        │  │
│  └──────────────────────────┘  └────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 FQC 质检状态卡片

```
┌── FQC 质检状态 ──────────────────────────────────────────────┐
│                                                              │
│  工序中含 2 个报检点：                                        │
│                                                              │
│  ┌── 工序: 注塑检验 (Step 1) ──────────────────────────────┐ │
│  │ 检验编号: INS-001        状态: ● 已完成                 │ │
│  │ 结果: ✗ 不合格            检验日期: 2024-06-14          │ │
│  │ [查看详情 →]                                             │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌── 工序: 成品全检 (Step 3) ──────────────────────────────┐ │
│  │ 检验编号: INS-002        状态: ● 已完成                 │ │
│  │ 结果: ✓ 合格              检验日期: 2024-06-15          │ │
│  │ [查看详情 →]                                             │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                              │
│  ⚠ 有 1 项不合格，无法确认入库                                │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**数据来源**：

```rust
// 查询工单的报检工序
let inspection_points: Vec<i64> = work_order_routings.iter()
    .filter(|r| r.is_inspection_point)
    .map(|r| r.id)
    .collect();

// 查询关联的检验结果
let fqc_results = state.inspection_result_service()
    .find_by_source(
        &service_ctx, &mut conn,
        InspectionSourceType::ProductionReceipt,  // 修正后的源类型
        receipt.id,
    ).await?;
```

**FQC 状态汇总计算**：

```rust
enum FqcGateStatus {
    NotRequired,              // 无报检点
    PendingInspection,        // 有报检点但无检验记录
    AllPassed,                // 全部合格
    HasFailed,                // 有不合格项
}

fn compute_fqc_gate(
    has_inspection_points: bool,
    results: &[InspectionResultSummary],
) -> FqcGateStatus {
    if !has_inspection_points {
        return FqcGateStatus::NotRequired;
    }
    if results.is_empty() {
        return FqcGateStatus::PendingInspection;
    }
    let all_passed = results.iter().all(|r| {
        r.status == InspectionStatus::Completed
            && r.result == InspectionResultType::Pass
    });
    if all_passed { FqcGateStatus::AllPassed } else { FqcGateStatus::HasFailed }
}
```

### 4.3 确认按钮 FQC 门控

```rust
fn confirm_button(receipt_status: ReceiptStatus, fqc_gate: &FqcGateStatus) -> Markup {
    if receipt_status != ReceiptStatus::Draft {
        return html! {};
    }
    match fqc_gate {
        FqcGateStatus::AllPassed | FqcGateStatus::NotRequired => {
            html! {
                form hx-post=(format!("/admin/mes/receipts/{}/confirm", receipt.id))
                      hx-swap="none" style="display:inline" {
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

### 4.4 成本明细卡片

```
┌── 成本明细 ──────────────────────────────────────────────────┐
│                                                              │
│  入库数量:    100                                             │
│  单位成本:    ¥15.00   （来源: stock_ledger 最后已知成本）    │
│  总成本:      ¥1,500.00                                       │
│                                                              │
│  ── 成本构成 ──                                               │
│  成本类型      │ 金额        │ 占比                          │
│  ● 材料       │ ¥1,200.00  │ 80.0%                         │
│  ● 人工       │ ¥200.00    │ 13.3%                         │
│  ● 制造费用   │ ¥100.00    │ 6.7%                          │
│  ────────────────────────────────────────────────────────    │
│  合计          │ ¥1,500.00  │ 100%                          │
│                                                              │
│  ── 成本分录 ──                                               │
│  借: WIP-工单 #MO-2024-001    ¥1,500.00                     │
│  贷: 成本转出                  ¥1,500.00                     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**实现**：

```rust
fn cost_detail_card(
    received_qty: Decimal,
    unit_cost: Decimal,
    cost_entries: &[CostEntrySummary],
    wo_id: i64,
) -> Markup {
    let total_cost = received_qty * unit_cost;
    html! {
        div class="data-card" {
            div class="data-card-head" {
                h3 { "成本明细" }
            }
            div class="info-grid" {
                div class="info-item" {
                    span class="info-label" { "入库数量" }
                    span class="info-value mono" { (fmt_qty(received_qty)) }
                }
                div class="info-item" {
                    span class="info-label" { "单位成本" }
                    span class="info-value mono" {
                        @if unit_cost > Decimal::ZERO {
                            (fmt_money(unit_cost))
                        } @else {
                            span class="muted" { "—（无历史成本）" }
                        }
                    }
                }
                div class="info-item" {
                    span class="info-label" { "总成本" }
                    span class="info-value mono" {
                        strong { (fmt_money(total_cost)) }
                    }
                }
                div class="info-item" {
                    span class="info-label" { "成本来源" }
                    span class="info-value muted" { "stock_ledger 最后已知成本" }
                }
            }
            // 成本分录列表
            @if !cost_entries.is_empty() {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "成本类型" }
                                th class="num-right" { "金额" }
                                th class="num-right" { "占比" }
                            }
                        }
                        tbody {
                            @for entry in cost_entries {
                                tr {
                                    td { (cost_type_label(entry.cost_type)) }
                                    td class="mono num-right" { (fmt_money(entry.amount)) }
                                    td class="mono num-right muted" {
                                        (format!("{:.1}%", entry.ratio(total_cost) * 100.0))
                                    }
                                }
                            }
                            tr class="total-row" {
                                td { strong { "合计" } }
                                td class="mono num-right" { strong { (fmt_money(total_cost)) } }
                                td {}
                            }
                        }
                    }
                }
            }
        }
    }
}
```

### 4.5 倒冲详情卡片

```
┌── 倒冲详情 ──────────────────────────────────────────────────┐
│                                                              │
│  倒冲状态: ✓ 已触发                                          │
│  倒冲时间: 2024-06-15 14:30                                  │
│                                                              │
│  ── 倒冲消耗物料 ──                                          │
│  产品       │ 倒冲数量 │ 单位成本 │ 金额      │ 仓库        │
│  原料A      │ 48 kg   │ ¥10.00  │ ¥480.00  │ 主仓库      │
│  原料B      │ 30 kg   │ ¥25.00  │ ¥750.00  │ 主仓库      │
│  螺丝M4     │ 200 pc  │ ¥0.50   │ ¥100.00  │ 主仓库      │
│  ────────────────────────────────────────────────────────    │
│  倒冲总消耗: ¥1,330.00                                       │
│                                                              │
│  [查看倒冲事务详情 →]                                        │
└──────────────────────────────────────────────────────────────┘
```

**数据来源**：查 `backflushes` 表 + 关联 `stock_ledger` 事务记录。

### 4.6 消除 inline style

当前代码用 `style="display:inline"` 和 `style="padding:2px 8px;..."` 违反 CLAUDE.md。

**修复**：
- `style="display:inline"` → 移到 form 的 CSS class `.inline-form`
- 状态 pill 的 inline style → 使用 `status-pill` CSS class + 变体类

## 5. Handler 数据加载增强

```rust
pub async fn get_receipt_detail(path: ReceiptDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_receipt_service();
    let receipt = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let lookups = svc.get_detail_lookups(&mut conn, &receipt).await?;

    // 新增查询
    // 1. 工单报检点
    let wo_routings = state.work_order_service()
        .get_routings(&service_ctx, &mut conn, receipt.work_order_id).await?;
    let has_inspection_points = wo_routings.iter().any(|r| r.is_inspection_point);

    // 2. FQC 检验结果
    let fqc_results = state.inspection_result_service()
        .find_by_source(&service_ctx, &mut conn, receipt.id).await?;
    let fqc_gate = compute_fqc_gate(has_inspection_points, &fqc_results);

    // 3. 成本数据
    let unit_cost = svc.get_unit_cost(&mut conn, receipt.product_id).await?;
    let cost_entries = state.cost_entry_service()
        .find_by_source(&service_ctx, &mut conn, DocumentType::ProductionReceipt, path.id).await?;

    // 4. 倒冲详情
    let backflush = if receipt.backflush_triggered {
        state.backflush_service()
            .find_by_receipt(&service_ctx, &mut conn, path.id).await.ok()
    } else {
        None
    };

    // 渲染
    let content = html! { ... };
    Ok(Html(admin_page(...).into_string()))
}
```

## 6. CSS 新增

```css
/* FQC 状态徽章 */
.fqc-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: var(--radius-pill);
    font-size: var(--text-xs);
    font-weight: 500;
}
.fqc-badge--pending { background: rgba(255, 159, 67, 0.08); color: var(--warning); }
.fqc-badge--passed { background: rgba(82, 196, 26, 0.08); color: var(--success); }
.fqc-badge--failed { background: rgba(245, 63, 63, 0.06); color: var(--danger); }
.fqc-badge--na { background: var(--gray-100); color: var(--muted); }

/* inline-form */
.inline-form { display: inline; }

/* total-row */
.total-row {
    border-top: 2px solid var(--border);
    font-weight: 600;
}
```

## 7. 实现步骤

1. `mes_receipt_detail.rs` 重构：
   - 页面结构从单卡片改为 Tab 结构
   - 新增 FQC 质检状态卡片
   - 新增成本明细卡片
   - 新增倒冲详情卡片
   - 确认按钮 FQC 门控
   - 消除 inline style
2. `get_receipt_detail` handler — 新增 4 项数据查询
3. `state.rs` — 确认 `inspection_result_service()` / `cost_entry_service()` / `backflush_service()` 可用
4. `base.css` — 加 FQC badge + total-row 样式

## 8. 验收标准

- [ ] FQC 质检状态卡片正确展示（通过/不通过/待检/不需要）
- [ ] 确认按钮在 FQC 未通过时禁用并显示原因
- [ ] 成本明细卡片展示 unit_cost / total_cost / 成本分录
- [ ] 倒冲详情卡片展示消耗物料列表
- [ ] 无 inline style 属性
- [ ] 工单/批次编号有跳转链接
- [ ] cargo clippy 零错误
