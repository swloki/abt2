# 从工艺路径加载：选路径 → 列工序 → 设产出品+单价 → 应用到工单

- 日期：2026-06-21
- 模块：`abt-web/src/components/routing_picker.rs`（新）、`abt-web/src/pages/mes_order_detail.rs`、`abt-core/src/mes/production_batch`
- 状态：待实现
- 承接：`2026-06-21-routing-product-batch-load-design.md`（其 `load_routings_from_template` 自动加载被本设计**替换**为选择+编辑+应用流）；`product_picker.rs`（picker 范式参考）

## 1. 背景与目标

「从工艺路径加载」当前是一键自动从工单自身 routing 加载产出品（模板多数未绑 → 加载为空）。用户要求改为：弹**工艺路径选择器**选一条路径 → 列出其工序 → 逐道设产出品+单价 → 应用到当前工单工序。把"绑定产出品/单价"做成可见可编辑的设计环节。

## 2. 关键决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 应用目标 | 工单工序（work_order_routings，按 step_no） | 用户确认；只影响当前工单 |
| 选择器 | 新建 routing_picker（镜像 product_picker） | 复用成熟范式；可复用 |
| 预填值 | 选中路径模板（routing_steps）的产出品+单价 | 模板已绑则预填，未绑则空让用户填 |
| 跨路径匹配 | 选中路径与工单工序按 step_no 匹配，不匹配跳过 | 用户自担对齐风险；实际多选工单自身路径 |
| 守卫 | 仅更新未报工工序行；单价>0；整单全报工按钮禁用 | 与现有产出品编辑守卫一致 |
| 取代 | `load_routings_from_template`（自动加载）被本流程替换 | 自动加载对未绑模板无效 |

## 3. 交互设计

### 3.1 触发
工单详情工序 tab「从工艺路径加载」按钮（已存在）：点击 → 打开 routing picker modal。

### 3.2 Routing 选择器（新组件 `routing_picker.rs`）
镜像 `product_picker_modal`：
- `routing_picker_modal(modal_id, target_id, display_id)`：搜索框（关键词）+ 结果列表
- 搜索端点：`GET /api/routings/search?keyword=X` → 返回路径列表（id + name），点击行 → 填 hidden `target_id` + 显示名到 `display_id` + 发 `routingSelected` 事件 + 关弹窗
- 复用 `RoutingService::list(keyword)`

### 3.3 选中后 → 应用抽屉
监听 `routingSelected`：`hx-get=/admin/mes/orders/{oid}/routings/apply-from-routing hx-include=#routing-id-hidden hx-target=#routing-apply-drawer-body`，`_="on 'htmx:afterRequest' add .open to #routing-apply-drawer"`。

**GET apply-from-routing?routing_id=R**：取 `RoutingService::get_detail(R)` 的 steps → 渲染抽屉表单：
```
<form id="routing-apply-form" hx-post=apply-from-routing hx-target=#routing-apply-drawer-body>
  每行（一个 step）：序号 + 工序名 + 产出品(product_picker，预填 step.product_id) + 单价(input，预填 step.unit_price)
  hidden: step_no
</form>
+ product_picker_modal（产出品选择）
```

### 3.4 保存 → 应用
**POST apply-from-routing**：解析表单（多行 step_no + product_id + unit_price）→ 调 `apply_routing_to_work_order` → 成功返回 OOB 刷新工序 tab + 关抽屉；失败返带错误表单。

## 4. 接口设计

### 4.1 Service（`ProductionBatchService`）
- **移除** `load_routings_from_template`（被取代）
- **新增** `apply_routing_to_work_order(ctx, db, work_order_id, items: Vec<RoutingStepApply>) -> Result<usize>`
  ```rust
  pub struct RoutingStepApply {
      pub step_no: i32,
      pub product_id: Option<i64>,
      pub unit_price: rust_decimal::Decimal,
  }
  ```
  实现（单事务）：
  1. 工单状态 ∈ {Released, InProduction}
  2. 对每个 item：找 work_order_routings 中 (work_order_id, step_no)；若存在且 `!has_report` 且 `unit_price > 0` → UPDATE product_id, unit_price
  3. 审计 `AuditAction::Update`（changes="应用工艺路径配置，{n}行"）
  4. 返回应用行数

### 4.2 Web
- **新组件** `routing_picker.rs`：`routing_picker_modal` + `GET /api/routings/search`（注册到 router）
- **端点** `OrderRoutingApplyFromRoutingPath = /admin/mes/orders/{order_id}/routings/apply-from-routing`（GET 表单 / POST 应用）
- handler：
  - `get_apply_from_routing(path, Query(routing_id))` → 抽屉表单
  - `post_apply_from_routing(path, Form<ApplyForm>)` → 调 service → OOB 刷新 + 关抽屉
- `ApplyForm`：多行 step_no/product_id/unit_price（用 ` serde` 平铺命名 `steps[]` 或 JSON hidden）
- **移除** `OrderRoutingLoadTemplatePath` + `load_routings_from_template` handler（按钮改触发 picker）

### 4.3 按钮改造
「从工艺路径加载」按钮：`_="on click add .is-open to #routing-picker-modal"`（开 picker，不再 hx-post 自动加载）。页面渲染 routing_picker_modal + 应用抽屉壳 `#routing-apply-drawer`（body `#routing-apply-drawer-body`）。

## 5. 数据流
```
「从工艺路径加载」→ 开 routing-picker-modal → 搜索/选 R
  → routingSelected → GET apply-from-routing?routing_id=R → 抽屉：R.steps × (产出品 picker + 单价，预填模板值)
  → 编辑 → POST → apply_routing_to_work_order 按 step_no 写 work_order_routings（未报工 + 单价>0）
  → OOB 刷新工序 tab + 关抽屉
```

## 6. 错误处理
| 场景 | 处理 |
|---|---|
| 单价 ≤ 0 | 该行跳过（不应用）+ 整体不报错；或 validation 拒绝整表 |
| step_no 在工单工序中不存在 | 跳过 |
| 工序已报工 | 跳过（不覆盖） |
| 状态不符 | `business_rule` |
| 未选路径就触发表单 | 前端 picker 强制先选 |

> 单价≤0 处理：采用「该行跳过」更宽松（允许只设产出品不设价）；若需严格，改 validation。默认跳过。

## 7. 测试（DB 集成，串行）
- `apply_routing_to_work_order`：模板设产出品+单价 → 应用 → 工单对应工序两字段正确；报工行不覆盖；不匹配 step_no 跳过；单价≤0 跳过
- `/api/routings/search`：关键词返回匹配路径
- GET apply-from-routing：返回含 steps + 预填值的表单 HTML
- 移除旧 `load_routings_from_template` 测试

## 8. 设计文档同步
`docs/uml-design/04-mes.html`：`ProductionBatchService` 把 `load_routings_from_template` 替换为 `apply_routing_to_work_order`。

## 9. 不做（YAGNI）
- 不改「从最近工单加载」（保持自动复制）
- 不改逐行编辑抽屉（精修保留）
- routing picker 不做多选（单选一条路径）
- 不做应用预览/dry-run
