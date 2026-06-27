# 生产作业中心 — 订单排期 + dp-toggle 对齐修复计划

**日期**：2026-06-27 | **范围**：MES / 生产作业中心（`/admin/mes/work-center`） | **原型**：`04-work-center-hub.html`

**用户决策**：① dp-toggle tab 对齐原型平铺风格（同页工单 card 同步） ② 订单排期完整对齐

---

## 总览

| 区块 | 检查项 | 原型 | 实现 | 浏览器差异 | 代码定位 | 🔴 | 🟡 |
|------|--------|------|------|-----------|---------|-----|-----|
| dp-toggle | A1 tab 容器形态 | 平铺 tab（底部下划线） | 分段控件（胶囊外壳） | ❌ | `mes_work_center.rs:274` | 🔴 | |
| dp-toggle | A2 "可合并"小字 | 有 | 无 | ❌ | `mes_work_center.rs:280` | | 🟡 |
| dp-toggle | A3 排序下拉 | 4 选项 | 无 | ❌ | 未实现（后端依赖） | | 🟡 |
| dp-toggle | A4 完整需求池链接 | 有 | 无 | ❌ | 未实现 | | 🟡 |
| 排期 | B1 表头列数 | 8 列 | 6 列 | ❌ | `mes_work_center.rs:660` | 🔴 | |
| 排期 | B2 来源列 | SO 链接 | 缺 | ❌ | `mes_work_center.rs:686` | 🔴 | |
| 排期 | B3 工作中心列 | 有 | 缺 | ❌ | `mes_work_center.rs:686` | 🔴 | |
| 排期 | B4 计划列名 | 计划开工→完工 | 计划日期 | ⚠️ | `mes_work_center.rs:664` | | 🟡 |
| 排期 | B5-B7 排期筛选 | 工作中心/状态/时间 | 仅搜索 | ❌ | `mes_work_center.rs:264` schedule 分支 | 🔴 | |
| 排期 | B8 逾期行高亮 | warn-bg + 红字 | 无 | ❌ | `mes_work_center.rs:686` | | 🟡 |
| 排期 | B9 状态文案 | 待下达/已排期 | 待计划/已计划 | ⚠️ | `mes_work_center.rs:718` | | 🟡 |

**整体匹配度：约 35%** | **待修复：11 项（🔴 5 / 🟡 6）**

---

## 后端改动（abt-core）

### H1. WorkOrderFilter 增加 work_center_id 筛选 — 🔴

- **位置**：`abt-core/src/mes/work_order/model.rs:55-63`（`WorkOrderFilter` struct）
- **改动**：新增 `pub work_center_id: Option<i64>` 字段
- **同步**：`abt-core/src/mes/work_order/repo.rs` list 过滤分支（约 `L130-145` where 构造 + `L186-195` bind），加 `if let Some(wc) = filter.work_center_id { where_clauses.push("wo.work_center_id = $n"); ... bind(wc) }`
- **依据**：repo.rs 已确认 list SELECT 填充 `source_so_doc`/`work_center_id`/`sales_order_id`/`source_customer`（L59-70），且已支持 `date_from/date_to/status/keyword/product_code` 过滤，仅缺 work_center_id
- **设计文档同步**：`docs/uml-design/` mes work_order 接口（WorkOrderFilter 定义处）

> 时间筛选复用已有 `date_from/date_to`，状态筛选复用已有 `status`，无需新增字段。

---

## 前端改动 A — dp-toggle 对齐原型平铺 tab

原型 CSS（`04-work-center-hub.html:65-72`）：
```css
.dp-toggle { display:flex; gap:8px; padding-bottom:12px; border-bottom:1px solid var(--border-soft) }
.dp-toggle-btn { padding:6px 14px; border:none; background:transparent; border-radius:4px;
                  font-size:14px; font-weight:500; color:var(--muted); cursor:pointer }
.dp-toggle-btn.active { background:var(--accent-bg); color:var(--accent); font-weight:600 }
.dp-tools { margin-left:auto; display:flex; gap:12px }  /* 排序+完整需求池 靠右 */
```

### A1. tab 容器改为平铺风格 — 🔴

- **位置**：`mes_work_center.rs:273-293`（`demand_filter_bar` 的 tab 容器 + `toggle_cls:325-331`）
- **现状**：`inline-flex bg-surface border border-border-soft rounded-md p-[3px]` 外壳胶囊包裹三按钮（分段控件）
- **改为**：去掉外壳胶囊，容器改 `flex items-center gap-2 border-b border-border-soft pb-3`；按钮用 `toggle_cls` 平铺（透明背景、`px-3.5 py-1.5 rounded-sm`，激活 `bg-accent-bg text-accent font-semibold`，未激活 `text-muted hover:text-fg`）
- **同步**：工单 card `orders_tabs_and_filter:842-857` 用同一套平铺 tab 类（抽公共 helper 或复用 `toggle_cls`），保证同页两 card 风格一致

### A2. 物料汇总 tab 加"可合并"小字 — 🟡

- **位置**：`mes_work_center.rs:280`
- **改动**：按钮内 "物料汇总" 后加 `span class="text-[10px] text-muted font-medium ml-0.5" { "可合并" }`

### A3. 排序下拉 — 🟡（后端依赖，建议拆二期）

- **原型**：`dp-tools` 内 `排序` label + `dp-sort` 下拉（紧急度/总需求量/最早交期/涉及订单数）
- **范围**：仅物料汇总 / 订单行明细视图显示（schedule 视图不显示，原型 schedule 用自己的 dp-searchbar）
- **后端依赖**：排序维度（紧急度/总需求量/涉及订单数）需 `MaterialAggQuery`/`DemandPoolQuery` 加 `sort` 参数 + repo `ORDER BY`
- **建议**：本轮先加 UI 下拉控件（htmx `hx-vals` 携带 sort），后端排序逻辑作为二期；或本轮一并补 sort 参数。**待用户确认是否本轮实现排序后端**

### A4. 完整需求池链接 — 🟡

- **位置**：`demand_filter_bar` 工具栏右侧（仅物料汇总/明细视图）
- **改动**：`a href="/admin/mes/demand-pool" class="text-xs text-accent font-semibold no-underline" { "完整需求池 →" }`（实现时确认需求池列表 TypedPath）

---

## 前端改动 B — 订单排期完整对齐

### B1+B4. 表头改 8 列 — 🔴

- **位置**：`mes_work_center.rs:659-667`（`render_schedule_table` thead）
- **改为**：`订单号 / 产品 / 来源 / 数量 / 工作中心 / 计划开工→完工 / 状态 / 操作`（8 列）

### B2. 来源列 — 🔴

- **位置**：`schedule_row:680-712` 加 td
- **数据**：`w.source_so_doc`（已 JOIN 填充）+ `w.sales_order_id`（做链接 `/admin/orders/{id}`）
- **渲染**：`@if let (Some(so), Some(oid)) = (&w.source_so_doc, w.sales_order_id) { a.link } @else if let Some(so)=... { 文本 } @else { "—" }`

### B3. 工作中心列 — 🔴

- **位置**：`schedule_row` 加 td；`get_demand_card` schedule 分支（`L175-201`）加 wc_map 解析
- **数据**：`w.work_center_id` → 名称（复用 release drawer 模式：`new_work_center_service(state.pool.clone()).list_active()` → `HashMap<i64,String>`）
- **传参**：`render_schedule_table` / `schedule_row` 签名加 `wc_map: &HashMap<i64,String>`

### B5-B7. 排期视图筛选栏 — 🔴

- **位置**：`demand_filter_bar` schedule 分支（`L267-315`，目前 schedule 仅显示搜索框）
- **改为**：搜索 + 工作中心下拉 + 状态下拉 + 时间下拉（对齐原型 dp-searchbar）
  - 工作中心：`list_active` 工作中心 → `name="work_center_id"`
  - 状态：`全部/待下达(Draft)/已排期(Planned)` → `name="sched_status"`
  - 时间：`全部/本周开工/已逾期/下周开工` → 计算 `date_from/date_to`（已逾期 = `date_to=today`）
- **参数**：`DemandCardParams`（`L120-131`）加 `work_center_id: Option<i64>`、`sched_status: Option<String>`
- **查询**：schedule 分支按参数构造 `WorkOrderFilter { work_center_id, status, date_from, date_to, keyword, .. }`（status 筛选时只取对应状态；不筛选取 Draft+Planned 合并）

### B8. 逾期行高亮 — 🟡

- **位置**：`schedule_row`
- **判定**：`w.scheduled_end < today` → 整行 `bg-warn-bg`，计划列追加 `text-danger "· 逾期"`，状态列显示"逾期"（danger）

### B9. 状态文案对齐 — 🟡

- **位置**：`wo_status_meta:715-725`
- **改动**：`Draft → ("待下达", "muted")`、`Planned → ("已排期", "accent")`（原型 status-draft=待下达、status-confirmed=已排期）
- **影响面**：`wo_status_meta` 被排期 + 工单 card 共用；工单 card 默认显示 InProduction/Released，不显示 Draft/Planned，故改 Draft/Planned 文案主要影响排期，安全

---

## 逐项修复清单（执行顺序）

| # | 严重度 | 项 | 文件 | 修复方式 |
|---|--------|----|------|----------|
| 1 | 🔴 | H1 后端 work_center_id 筛选 | `model.rs:55` + `repo.rs:130/186` | Filter 加字段 + WHERE/bind |
| 2 | 🔴 | B1+B4 表头 8 列 | `mes_work_center.rs:659` | thead 改 8 列 |
| 3 | 🔴 | B2 来源列 | `mes_work_center.rs:686` | schedule_row 加 td（source_so_doc） |
| 4 | 🔴 | B3 工作中心列 | `mes_work_center.rs:175,686` | wc_map 解析 + schedule_row 加 td |
| 5 | 🔴 | B5-B7 排期筛选 | `mes_work_center.rs:120,267` | DemandCardParams 加参 + schedule 筛选栏 + Filter 构造 |
| 6 | 🔴 | A1 平铺 tab | `mes_work_center.rs:273,325,842` | 去外壳 + toggle_cls 平铺 + 工单 card 同步 |
| 7 | 🟡 | B8 逾期高亮 | `mes_work_center.rs:686` | scheduled_end<today 判定 |
| 8 | 🟡 | B9 状态文案 | `mes_work_center.rs:718` | Draft→待下达 / Planned→已排期 |
| 9 | 🟡 | A2 可合并小字 | `mes_work_center.rs:280` | span 小字 |
| 10 | 🟡 | A4 完整需求池链接 | `mes_work_center.rs:264` | a 链接 |
| 11 | 🟡 | A3 排序下拉 | `mes_work_center.rs:264` + 后端 | UI 下拉（后端 sort 二期/本轮，待确认） |

---

## 涉及文件

- `abt-core/src/mes/work_order/model.rs`（WorkOrderFilter 加 work_center_id）
- `abt-core/src/mes/work_order/repo.rs`（list 加 work_center_id 过滤）
- `abt-web/src/pages/mes_work_center.rs`（dp-toggle、排期表、筛选、状态文案）
- `docs/uml-design/`（mes work_order WorkOrderFilter 定义同步）

## 验证

- `cargo clippy -p abt-core` + `cargo clippy -p abt-web`
- agent-browser（`--cdp 9222`）：切到订单排期 tab → snapshot 确认 8 列 + 来源/工作中心 + 筛选 + 逾期高亮；dp-toggle snapshot 确认平铺 tab + 可合并小字 + 完整需求池链接

## 备注 / 决策点

1. **排期状态语义**：原型排期视图状态含"待下达/已排期/逾期"，并暗示可看生产中/待入库。本计划保持排期视图聚焦"待下达（Draft+Planned）"（与现实现定位一致，避免与工单 card 重叠），状态筛选提供 待下达/已排期 细分，逾期由 `scheduled_end<today` 独立判定。如需排期视图也展示生产中/待入库工单，需扩展 schedule 查询范围（额外工作量）。
2. **A3 排序下拉**：完整功能需后端 sort 参数（MaterialAggQuery/DemandPoolQuery + repo ORDER BY）。本轮是否实现后端排序待用户确认；否则先加 UI 占位。
3. **同页一致性**：A1 改 dp-toggle 平铺风格时，同步改工单 card `orders_tabs_and_filter`，避免同页两 card tab 风格不一。
