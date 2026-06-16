# UI-02: 工单详情增强

> 核心改动牵涉：文档②（工单下达完善）— RoutingStep 属性继承、close 95% 门控、cancel 领料单取消
> Odoo 参考：`mrp.production` 表单视图

## 1. 目标

工单详情页 (`mes_order_detail.rs`) 需要展示核心改动后的新数据维度，并在操作按钮上实现业务规则门控。

## 2. 当前状态

### 已有（无需改动）

- tab_routing 已展示 work_center_id / standard_time / is_outsourced / is_inspection_point
- 5 个 Action 按钮：Release / Unrelease / Close / Cancel / Split（按状态条件渲染）
- Tab 结构：信息 / 工序 / 批次 / 报工 / 日志

### 缺失（本次需补齐）

| 差距 | 说明 | 来源文档 |
|------|------|----------|
| 完工率展示 | 缺 completed_qty / planned_qty 比例条 | 文档② close 95% 门控 |
| actual_start/end | 缺实际开始/结束时间 | 文档④ 时间戳维护 |
| close 门控 | close 按钮无完工率 ≥95% 前置校验 | 文档② |
| cancel 警告 | cancel 无"有入库单时不可取消"提示 | 文档② |
| FQC 状态 | 缺当前工单 FQC 质检状态 | 文档⑤ FQC 门控 |
| 工序成本属性 | routing tab 缺 standard_cost / unit_price / allowed_loss_rate | 文档② 完整映射 |
| 物料消耗 | 缺关联领料单 Tab | 文档③ 领料完善 |

## 3. Odoo 参考

### `mrp.production` 表单

```
┌── 头部 ───────────────────────────────────────────┐
│ Manufacturing Order: WH/2024/001     [状态: 进行中] │
│ Product: [产品名]    To Produce: 100               │
│ Produced: 95 ████████████████████░░░ 95%           │
│ Reserved: ✓ Available                               │
├── Action Bar ──────────────────────────────────────┤
│ [Confirm] [Plan] [Check Availability] [Produce]    │
├── Notebook ────────────────────────────────────────┤
│ │ Components │ Work Orders │ Byproducts │ ... │    │
│ │                                            │    │
│ │ Components:                                │    │
│ │   Product | To Consume | Consumed | Unit  │    │
│ │   原料A   | 50         | 48      | kg    │    │
│ │                                            │    │
│ │ Work Orders:                               │    │
│ │   Operation | WC | Duration | State        │    │
│ │   注塑      | W1 | 2h       | Done         │    │
└──────────────────────────────────────────────────────┘
```

**关键 Odoo 模式**：
1. **Produced / To Produce 进度条** — header 直接显示完工比例
2. **Check Availability** — 校验物料是否预留
3. **Work Orders Tab** — 工序执行进度，含 Duration / Status
4. **Components Tab** — 物料消耗，To Consume vs Consumed 对比

## 4. 修改设计

### 4.1 tab_info — 补齐完工率和时间戳

```
┌── 基本信息 ──────────────┐  ┌── 生产配置 ──────────────┐
│ 工单编号: MO-2024-001    │  │ BOM 快照: #12            │
│ 产品: 电源板A            │  │ 工艺路线: #5             │
│ 计划数量: 100            │  │ 工序数: 5                │
│ 状态: ● 生产中           │  │ 物料模式: 推式           │
│ 版本号: v1               │  │                          │
│ 计划开始: 2024-06-15 08:00│ │ 创建时间: ...            │
│ 计划结束: 2024-06-20 18:00│ │                          │
└──────────────────────────┘  └──────────────────────────┘
┌── 生产进度 ──────────────────────────────────────────┐
│ 计划: 100  已完工: 95  完工率: ████████████████░ 95% │
│ 实际开始: 2024-06-15 08:30                           │
│ 实际结束: — (进行中)                                 │
│ FQC 质检: ⏳ 进行中 (2/3 通过)                       │
└──────────────────────────────────────────────────────┘
```

**实现**：

```rust
// 新增 tab_info 内的"生产进度"区块
fn production_progress_section(order: &WorkOrder, completed_qty: Decimal, fqc_status: FqcStatus) -> Markup {
    let completion_rate = if order.planned_qty > Decimal::ZERO {
        completed_qty / order.planned_qty
    } else {
        Decimal::ZERO
    };
    let pct = (completion_rate * Decimal::from(100)).to_string();
    html! {
        div class="info-section" {
            div class="info-section-title" { "生产进度" }
            div class="progress-section" {
                div class="progress-stats" {
                    span class="info-item" {
                        span class="info-label" { "计划" }
                        span class="info-value mono" { (fmt_qty(order.planned_qty)) }
                    }
                    span class="info-item" {
                        span class="info-label" { "已完工" }
                        span class="info-value mono" { (fmt_qty(completed_qty)) }
                    }
                    span class="info-item" {
                        span class="info-label" { "完工率" }
                        span class="info-value mono" { (pct) "%" }
                    }
                }
                // CSS 进度条
                div class="progress-bar-wrap" {
                    div class="progress-bar-fill"
                         style=(format!("width: {}%", pct.min("100")))
                    {}
                }
                // 时间戳
                div class="info-grid" {
                    @if let Some(start) = order.actual_start {
                        div class="info-item" {
                            span class="info-label" { "实际开始" }
                            span class="info-value mono" { (fmt_dt(start)) }
                        }
                    }
                    @if let Some(end) = order.actual_end {
                        div class="info-item" {
                            span class="info-label" { "实际结束" }
                            span class="info-value mono" { (fmt_dt(end)) }
                        }
                    } @else {
                        div class="info-item" {
                            span class="info-label" { "实际结束" }
                            span class="info-value muted" { "—（进行中）" }
                        }
                    }
                }
                // FQC 状态
                (fqc_status_badge(&fqc_status))
            }
        }
    }
}
```

**FqcStatus 枚举**（本地计算，不需要新 model）：

```rust
enum FqcStatus {
    NotRequired,           // 工序无报检点
    Pending,               // 有报检点但无检验记录
    InProgress { pass: usize, total: usize },
    Passed,
    Failed,
}
```

**数据来源**：查 `work_order_routings WHERE is_inspection_point = true` 得到报检工序，再查 `inspection_results` 关联。

### 4.2 tab_routing — 补齐成本属性列

当前列：序号 / 工序名称 / 工作中心 / 计划量 / 标准工时 / 委外 / 标记

**新增列**：

| 新列 | 数据 | 说明 |
|------|------|------|
| 标准成本 | `r.standard_cost` | Decimal，对标 Odoo workcenter.costs_hour × standard_time |
| 计件单价 | `r.unit_price` | Decimal，中国计件工资模式 |
 | 损耗容差 | `r.allowed_loss_rate` | 百分比 |

**替换后**：

```
序号 │ 工序名称 │ 工作中心 │ 计划量 │ 标准工时 │ 标准成本 │ 计件单价 │ 委外 │ 标记
 1   │ 注塑     │ WC001    │ 100   │ 2.0h    │ ¥160    │ ¥0.50   │ —    │ 报检
 2   │ 组装     │ WC002    │ 100   │ 1.5h    │ ¥180    │ ¥1.00   │ —    │ —
 3   │ 检测     │ WC003    │ 100   │ 0.5h    │ ¥30     │ —       │ —    │ 报检
```

**工作中心名称解析**：当前显示 `#123`（ID），需改为名称。在 handler 中预加载 work_center 名称 HashMap。

### 4.3 新增 tab_materials — 物料消耗 Tab

```
[信息] [工序] [批次] [报工] [物料消耗] [日志]
                              ^^^^^^^^ 新增
```

**内容**：

```
┌── 关联领料单 ──────────────────────────────────────┐
│ 领料单号     │ 状态     │ 物料行数 │ 发料金额       │
│ REQ-001     ● 部分发料 │ 5       │ ¥2,400.00     │
│ REQ-002     ● 已发料   │ 3       │ ¥1,800.00     │
├───────────────────────────────────────────────────┤
│ 物料明细                                           │
│ 产品       │ 需求量 │ 已发量 │ 工序 │ 单位成本    │
│ 原料A      │ 50    │ 48    │ 注塑 │ ¥10.00     │
│ 原料B      │ 30    │ 30    │ 注塑 │ ¥25.00     │
│ 螺丝M4     │ 200   │ 0     │ 组装 │ —          │
└───────────────────────────────────────────────────┘
[+ 创建领料单]  [退料]
```

**数据来源**：
- 通过 `document_links` 查关联领料单
- 每张领料单的 items（含 operation_id / batch_id）
- 从 `inventory_reservation` 查预留状态

**实现**：

```rust
fn tab_materials(
    requisitions: &[MaterialReqSummary],
    material_items: &[MaterialItemSummary],
    wo_id: i64,
) -> Markup {
    html! {
        // 领料单列表
        div class="data-card" {
            div class="data-card-head" {
                h3 { "关联领料单" }
                a class="btn btn-sm btn-primary"
                  href=(format!("/admin/wms/requisitions/new?wo_id={}", wo_id))
                  { "+ 创建领料单" }
            }
            table class="data-table" {
                thead { tr { th {"领料单号"} th {"状态"} th {"行数"} th class="num-right" {"金额"} th {"操作"} } }
                tbody {
                    @for req in requisitions {
                        tr {
                            td class="mono" { a href=(req.detail_url) { (req.doc_number) } }
                            td { (req_status_pill(req.status)) }
                            td class="mono num-right" { (req.item_count) }
                            td class="mono num-right" { (fmt_money(req.total_cost)) }
                            td { a href=(req.detail_url) { "查看" } }
                        }
                    }
                }
            }
        }
        // 物料明细
        div class="data-card" {
            h3 { "物料消耗明细" }
            table class="data-table" {
                thead { tr { th{"产品"} th class="num-right"{"需求量"} th class="num-right"{"已发量"} th{"工序"} th class="num-right"{"单位成本"} } }
                tbody {
                    @for item in material_items {
                        tr {
                            td { (item.product_name) }
                            td class="mono num-right" { (fmt_qty(item.required_qty)) }
                            td class="mono num-right" {
                                @if item.issued_qty < item.required_qty {
                                    span class="text-warning" { (fmt_qty(item.issued_qty)) }
                                } @else {
                                    span class="text-success" { (fmt_qty(item.issued_qty)) }
                                }
                            }
                            td class="mono" { (item.operation_name) }
                            td class="mono num-right" { (fmt_money(item.unit_cost)) }
                        }
                    }
                }
            }
        }
    }
}
```

### 4.4 Action 按钮门控

#### Close 按钮 — 95% 完工率门控

当前：`status == InProduction` 时显示 close 按钮，点击直接请求。

改为：前端根据完工率条件渲染，后端做最终校验（已有）。

```rust
// 在 action_buttons 中
RequisitionStatus::InProduction => {
    let completion_rate = completed_qty / order.planned_qty;
    let can_close = completion_rate >= Decimal::new(95, 2); // 0.95
    html! {
        @if can_close {
            button class="btn btn-primary"
                hx-post=(close_path.to_string())
                hx-confirm="确认关闭工单？所有批次必须已完工或已取消。"
                hx-redirect=(detail_path) {
                "关闭工单"
            }
        } @else {
            button class="btn btn-primary"
                disabled
                title=(format!("完工率 {}%，需 ≥ 95% 才能关闭", pct_str)) {
                "关闭工单（完工不足）"
            }
        }
    }
}
```

#### Cancel 按钮 — 入库单阻止提示

```rust
fn cancel_button(has_receipts: bool, cancel_path: &str, detail_path: &str) -> Markup {
    if has_receipts {
        html! {
            button class="btn btn-danger" disabled
                title="存在已确认的完工入库单，无法取消" {
                "取消（有入库记录）"
            }
        }
    } else {
        html! {
            button class="btn btn-danger"
                hx-post=(cancel_path)
                hx-confirm="确认取消工单？将同时取消关联领料单。"
                hx-redirect=(detail_path) {
                "取消工单"
            }
        }
    }
}
```

## 5. Handler 数据加载增强

`get_order_detail` handler 需增加以下查询：

```rust
// 1. 完工量汇总
let completed_qty = state.production_receipt_service()
    .sum_received_qty_by_work_order(&mut conn, order.id).await?;

// 2. FQC 状态
let fqc_status = compute_fqc_status(&state, &mut conn, &routings).await?;

// 3. 关联领料单
let requisitions = state.material_requisition_service()
    .find_by_source(&service_ctx, &mut conn, DocumentType::WorkOrder, order.id).await?;

// 4. 物料明细汇总
let material_items = aggregate_material_items(&requisitions, &product_names);

// 5. work_center 名称 Map
let wc_names = state.work_center_service()
    .get_names_by_ids(&mut conn, &wc_ids).await?;

// 6. 是否有入库单
let has_receipts = state.production_receipt_service()
    .has_confirmed_receipts(&mut conn, order.id).await?;
```

## 6. CSS 新增

`base.css` 新增进度条样式：

```css
.progress-bar-wrap {
    height: 8px;
    background: var(--gray-100);
    border-radius: var(--radius-pill);
    overflow: hidden;
}
.progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--primary), var(--primary-light));
    border-radius: var(--radius-pill);
    transition: width 0.3s ease;
}
```

## 7. 实现步骤

1. handler `get_order_detail` — 加 6 项数据查询
2. `tab_info` — 新增"生产进度"区块（完工率 + 时间戳 + FQC）
3. `tab_routing` — 新增 3 列（standard_cost / unit_price / allowed_loss_rate）
4. `tab_materials` — 新增 Tab 面板
5. `action_buttons` — close 门控 + cancel 警告
6. `base.css` — 加 progress-bar 样式
7. `state.rs` — 确认 `work_center_service()` / `material_requisition_service()` 可用

## 8. 验收标准

- [ ] tab_info 显示完工率进度条 + actual_start/end + FQC 状态
- [ ] tab_routing 显示 standard_cost / unit_price / allowed_loss_rate
- [ ] tab_materials 显示关联领料单 + 物料消耗明细
- [ ] 完工率 < 95% 时 close 按钮禁用
- [ ] 有入库单时 cancel 按钮禁用
- [ ] cargo clippy 零错误
