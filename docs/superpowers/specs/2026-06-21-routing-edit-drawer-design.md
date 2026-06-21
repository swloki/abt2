# 工单工序编辑抽屉（替换行内编辑）

- 日期：2026-06-21
- 模块：`abt-web/src/pages/mes_order_detail.rs` + `abt-core/src/mes/production_batch`
- 状态：待实现
- 背景：上轮在工单详情工序列表加了「行内编辑」（产出品/单价是表格里的 `hx-post` input）。实测 UX 差：产出品是裸数字 input（看不到/搜不到产品名）、单价+产出品两个 input 同行各自 `change` 触发整行 `outerHTML` 重渲染互相打断、行过宽。改为「行只读 + 编辑抽屉」。

## 1. 目标

把工单工序的产出品 + 计件单价编辑，从表格行内 input 改为：行只读展示 + 「编辑」按钮打开右侧抽屉，抽屉内用正式 product picker 选产出品 + number 输入单价，一次性保存。

## 2. 关键决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 触发方式 | 每行「编辑」按钮 → 页面级单个抽屉 | 复用既有 `drawer` 组件 + `product_list.rs` 范式；避免每行一个抽屉 |
| 抽屉字段 | 产出品（product picker）+ 计件单价 | 聚焦，与业务一致；standard_time/cost 保持只读（YAGNI） |
| 产出品选择 | 复用 `product_picker_modal`（fill-input 模式） | 已有可搜索产品弹窗，替代裸数字框 |
| Service 合并 | 上轮的 `update_routing_unit_price` + `update_routing_product` 合并为一个 `update_routing` | 抽屉一次提交两字段，单事务更干净；消除三套更新接口 |
| 行产出品展示 | 显示产品名（get_order_detail 批量解析） | 告别 `#id` 裸数字 |
| 守卫 | 不变（状态 ∈ {Released,InProduction}、属单、未报工、单价>0、审计） | 与上轮一致 |

## 3. UI 设计

### 3.1 表格行（只读 + 操作）
`tab_routing` / `routing_row_fragment` 改造：
- 每行 `<tr>` 加 `id="routing-row-{rid}"`（供 OOB 刷新）
- 产出品列、计件单价列 → **只读文本**：产出品显示产品名（无则 `—`）；单价 `¥X`（无则 `—`）
- 操作列：未报工行 = **「编辑」按钮**（`icon::edit_icon`）+ 删除按钮；报工行 = 两者隐藏（锁）
- 编辑按钮：`hx-get=/admin/mes/orders/{oid}/routings/{rid}/edit hx-target="#routing-edit-drawer-body" hx-swap="innerHTML"` + `_="on 'htmx:afterRequest' add .open to #routing-edit-drawer"`

### 3.2 编辑抽屉（页面级一个）
`order_detail_page` 渲染抽屉壳（复用 `components::drawer::drawer`）：
- `drawer_id="routing-edit-drawer"`，`form_id="routing-edit-form"`，submit_label="保存"
- body 容器 `id="routing-edit-drawer-body" _="on htmx:afterSettle add .open to #routing-edit-drawer"`（HTMX 载入表单后自动开抽屉）

### 3.3 GET /edit（返回抽屉表单）
`<form id="routing-edit-form" hx-post=/admin/mes/orders/{oid}/routings/{rid}/edit hx-target="#routing-edit-drawer-body" hx-swap="innerHTML">` 含：
- **产出品**：`product_picker_field` 风格的触发按钮 + hidden `name="product_id" id="routing-product-id"`（预填当前 product_id）+ 显示 span `id="routing-product-display"`（预填当前产品名）+ 渲染 `product_picker_modal("routing-product-modal", "routing-product-id", "routing-product-display")`
- **计件单价**：`<input type="number" step="any" name="unit_price" value={当前值} required>`
- 抽屉标题：`编辑工序 {step_no} - {process_name}`

### 3.4 POST /edit（保存 + 刷新行 + 关抽屉）
- 调 `update_routing(wo_id, rid, product_id, unit_price)`
- 成功 → 返回：更新后的 `<tr id="routing-row-{rid}" hx-swap-oob="true">...</tr>`（OOB 刷回表格）+ 关抽屉（响应末尾 `<script>document.querySelector('#routing-edit-drawer').classList.remove('open')</script>`）
- 失败（单价≤0 / 守卫拒绝）→ 返回重新渲染的表单（带错误提示，`hx-target=#routing-edit-drawer-body` 原地替换，**不关抽屉**）

## 4. 接口设计

### 4.1 Service（abt-core，合并）
`ProductionBatchService`：
- **移除** `update_routing_unit_price`、`update_routing_product`
- **新增** `update_routing(ctx, db, work_order_id, routing_id, product_id: Option<i64>, unit_price: Decimal) -> Result<WorkOrderRouting>`

实现（单事务，守卫按序）：
1. `unit_price > 0`，否则 `validation("计件单价必须大于 0")`
2. routing 属该 work_order（否则 `not_found`）
3. 工单状态 ∈ {Released, InProduction}（否则 `business_rule`）
4. 事务内 `!has_report(routing_id)`（否则 `business_rule("该工序已报工，不可修改")`）
5. UPDATE work_order_routings SET product_id=$2, unit_price=$3 WHERE id=$1
6. 审计 `AuditAction::Update`，changes=`"product_id: {old}→{new}; unit_price: {old}→{new}"`
7. 返回更新后的 WorkOrderRouting

### 4.2 Web（abt-web）
- **移除** `OrderRoutingPricePath`、`OrderRoutingProductPath`（TypedPath + 路由注册 + handler `update_routing_price`/`update_routing_product` + `RoutingPriceForm`/`RoutingProductForm`）
- **新增** `OrderRoutingEditPath = /admin/mes/orders/{order_id}/routings/{routing_id}/edit`（GET 表单 / POST 保存），注册 GET+POST
- handler：
  - `get_routing_edit(path, ctx) -> Html`：取 routing + 解析当前产出品名（product_service.get_by_ids），渲染抽屉表单 fragment
  - `post_routing_edit(path, ctx, Form<RoutingEditForm>) -> Html`：调 `update_routing`，成功返 OOB 行 + 关抽屉脚本；失败返带错误的表单
- `RoutingEditForm { #[serde(default, empty_as_none)] product_id: Option<i64>, unit_price: Decimal }`

### 4.3 行渲染签名
`routing_row_fragment(r, is_reported_step, order_has_report, product_name: Option<&str>)` —— 加 product_name 参数（行展示产出品名）。`routing_tbody_fragment` 同步加 `product_names: &HashMap<i64,String>`。`get_order_detail` 批量取 routing 的 product_id → `product_service.get_by_ids` → 构造 map 传入。

## 5. 数据流
```
行「编辑」按钮 --hx-get--> GET /edit → 抽屉表单（product picker 预填 + 单价预填）→ afterSettle 开抽屉
抽屉 submit --hx-post--> POST /edit → update_routing（单事务，守卫）
                                       ├─ 成功: <tr hx-swap-oob> 刷回表格 + 关抽屉
                                       └─ 失败: 表单带错误原地替换（不关）
```

## 6. 错误处理
| 场景 | 处理 |
|---|---|
| 单价 ≤ 0 | `validation`，表单返回带错误，不关抽屉 |
| 已报工 | `business_rule`，同上 |
| 越权/不存在 | `not_found` |
| 并发（报工与编辑竞争） | 事务内 `has_report` 复查 |

## 7. 测试（DB 集成，`abt-web/tests/mes_routing_price.rs`，串行）
- 更新 `service_update_routing`：正常保存 product_id + unit_price（一次调，两字段都持久化）；守卫各分支（单价≤0、跨单、状态不符、已报工）
- 移除旧测试 `service_update_price_rejects_zero` / `service_update_price_ok_then_persists` / `service_update_price_rejects_cross_order` / `service_update_routing_product_*`（替换为统一的 service_update_routing 用例）
- GET /edit：返回 HTML 含 product picker + 预填单价
- POST /edit：成功后行 OOB 刷新（HTML 含 `routing-row-{rid}` + 关抽屉脚本）；单价≤0 返回带错误表单

## 8. 设计文档同步
`docs/uml-design/04-mes.html`：`ProductionBatchService` 把 `update_routing_unit_price`/`update_routing_product` 替换为 `update_routing`。

## 9. 不做（YAGNI）
- 不在抽屉编辑 standard_time/standard_cost（保持只读）
- 不改删除流程（仍走原 delete 端点 + 守卫）
- 不改报工带价链路（wage 冻结不变）
