# Issue #67 委外单创建页 + 工序产出品 product_id

- 日期：2026-06-21
- 关联：GitHub Issue #67 `[OM委外] 委外单创建页功能优化`；草案 `doc/2026-06-21-issue67-outsourcing-create-design.md`
- 模块：`master_data/routing`、`mes/production_batch`、`master_data/bom`、`master_data/product`、`om/outsourcing_order`、`abt-web/pages/{routing_create,routing_detail,mes_order_detail,om_outsourcing_create}`
- 状态：待实现
- 依赖：承接本分支已完成的「工单工序计件单价维护 + 删除 + wage 冻结」（`work_order_routings` 已有维护 UI 与守卫）

## 1. 背景与目标

委外单创建页（`/admin/om/outsourcing/create`）三个核心需求：
1. 选「关联工单」自动带出 产品/计划数量/交期/客户
2. 「关联工序」由裸数字输入框 → 显示工序名的下拉
3. 选「产品+工序+数量」后即时带出 物料/名称/需求量/库存；需求量须能被 `min_pack_qty` 整除

需求 3 的链路核心是「工序 → 它加工的半成品 → 半成品的 BOM → 物料」，而当前 `work_order_routings` 与 `routing_steps` 都**没有 product_id**（无法知道一道工序产出哪个半成品）。趁本分支还在动 `work_order_routings`（已加 unit_price/删除/wage 冻结），把 `product_id` 一起加上，避免二次返工。

## 2. 关键决策

| 决策 | 选择 | 理由 |
|---|---|---|
| product_id 表位置 | `routing_steps`（模板）+ `work_order_routings`（实例快照），下达时传播 | 半成品是工序固有属性，模板级最语义正确、可复用；与 unit_price 同链 |
| 实例 product_id 可编辑 | 是，首次报工前可改（复用 unit_price 守卫） | 兜底模板未填 + 允许工单级覆盖；与现有维护 UX 一致 |
| min_pack_qty 存储 | `products.min_pack_qty` 独立列 | 与 unit 等物料属性并列，便于查询/校验 |
| 委外工序下拉 | 默认只列 `is_outsourced=true`，「全部」checkbox 切换 | 业务上只委外标记为可委外的工序 |
| 发料库存取仓 | 表单 `source_warehouse_id`，未选取全仓库合计 | 与发料源一致 |
| suggest 物料挂哪 | `BomService`（BOM 展开是 BOM 域职责） | 通用，未来别处可复用；库存由 OM 层叠加 |

## 3. 数据模型变更（migration 063）

```sql
ALTER TABLE routing_steps       ADD COLUMN IF NOT EXISTS product_id   BIGINT REFERENCES products(product_id);
ALTER TABLE work_order_routings ADD COLUMN IF NOT EXISTS product_id   BIGINT REFERENCES products(product_id);
ALTER TABLE products            ADD COLUMN IF NOT EXISTS min_pack_qty DECIMAL(18,6);
ALTER TABLE outsourcing_orders  ADD COLUMN IF NOT EXISTS process_name VARCHAR(200);
```

回填：均为可空/默认 0，旧数据兼容（`#[sqlx(default)]`）。

## 4. Phase A — 数据基础

### A1. 模型 / repo
- `RoutingStep`、`WorkOrderRouting`、`Product`、`OutsourcingOrder` 加字段（`#[sqlx(default)]`）。
- 对应 repo 的 SELECT/INSERT/UPDATE 加列：
  - `routing_steps`：repo INSERT/UPDATE（`RoutingStepInput` 加 `product_id`）、SELECT
  - `work_order_routings`：`WorkOrderRoutingRepo` 的 insert/SELECT/get_by_* 加 `product_id`
  - `products`：SELECT 加 `min_pack_qty`
  - `outsourcing_orders`：INSERT/UPDATE/SELECT 加 `process_name`

### A2. 下达快照传播
`WorkOrderService::release`（`abt-core/src/mes/work_order/implt.rs`，深拷贝 routing_steps → work_order_routings 处）加 `product_id: step.product_id`（与 `unit_price` 同处）。

### A3. 维护 UI
- **模板级（主）**：`abt-web/src/pages/routing_create.rs` 工序行加「产出品」product picker（复用 `entity_picker` 组件，`target_id="product_id"`）；`routing_detail.rs` 展示产出品名。
- **实例级（兜底/覆盖）**：工单详情工序列表（`mes_order_detail.rs::tab_routing`，本分支已建）加「产出品」列：
  - 未报工行 → product picker（`hx-post` 存，复用 unit_price 的端点范式，新增 product 更新端点）
  - 报工行 → 只读文本
  - 守卫与 unit_price 同（首次报工前可改、报工后锁）

> 实例 product_id 编辑需要一个新端点（`/admin/mes/orders/{oid}/routings/{rid}/product`，POST `product_id`），与 `update_routing_unit_price` 同范式同守卫。

## 5. Phase B — 委外单创建页（`abt-web/src/pages/om_outsourcing_create.rs`）

### B1. 基本信息联动（需求1）
- 关联工单 `change` → `hx-get /admin/om/outsourcing/wo-summary?wo_id=X` → 回填 `product_id`/`planned_qty`/`scheduled_date`(=wo.scheduled_end)/客户名（只读 `source_customer`）
- 选「产品」时按 product 过滤「关联工单」下拉（前端 `hx-vals` 带过滤）

### B2. 关联工序下拉（需求2）
- 替换 `<input name="routing_id">` 为 `<select>`
- 选项 = 所选工单 `work_order_routings`（`ProductionBatchService::list_routings` 已含 `process_name`/`unit_price`/`product_id`/`is_outsourced`）
- 默认只列 `is_outsourced=true`，一个 checkbox「显示全部工序」切换
- 提交值 = `work_order_routings.id`；同时把所选工序 `process_name` 存入 `outsourcing_orders.process_name`

### B3. 发料明细联动 + min_pack_qty 校验（需求3）
- 选定「工序 + 计划数量」后 → `hx-get /admin/om/outsourcing/suggest-materials?routing_id=R&planned_qty=Q&warehouse_id=W` → 渲染物料行表格（编码/名称/需求量/库存/min_pack_qty）
- 需求量 = `bom_node.quantity × (1+loss_rate) × planned_qty`
- 库存 = `source_warehouse_id` 库存，未选取全仓库合计
- min_pack_qty 校验（纯前端 JS）：物料行 input 失焦 + 表单 submit 时校验 `qty % min_pack_qty == 0`；不满足红字提示「需求数量必须是最小包装数量 [X] 的整数倍」+ `halt` 提交
- 用户可在自动带出的基础上手动增删/改量，最终仍受 min_pack_qty 校验

## 6. Service 接口（abt-core，接口先行）

### 6.1 复用
- `ProductionBatchService::list_routings(ctx, db, work_order_id) -> Vec<WorkOrderRouting>`（B2 用，已含全部所需字段）

### 6.2 新增 — `OutsourcingOrderService`
```rust
pub struct WorkOrderOutsourcingSummary {
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub scheduled_end: Option<NaiveDate>,
    pub customer_name: Option<String>,
    pub routings: Vec<WorkOrderRouting>,   // 默认仅 is_outsourced=true 也一并返回，前端筛
}
async fn outsourcing_summary(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64
) -> Result<WorkOrderOutsourcingSummary>;
```

### 6.3 新增 — `BomService`（BOM 展开职责在此）
```rust
pub struct MaterialSuggestionItem {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub required_qty: Decimal,       // bom用量×(1+loss_rate)×planned_qty
    pub min_pack_qty: Option<Decimal>,
}
/// 按产出半成品 + 计划数量展开 BOM 物料需求（BOM 域纯净：不含库存）
async fn suggest_materials(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>,
    product_id: i64, planned_qty: Decimal,
) -> Result<Vec<MaterialSuggestionItem>>;
```
内部：`find_published_bom_by_product_code` → `get_leaf_nodes`/`explode_for_procurement` × planned_qty → JOIN `products` 取 code/name/min_pack_qty。

**库存叠加在 OM 层**（避免 Bom→WMS 跨域耦合）：abt-web 的 suggest-materials handler 先 `list_routings` 取 routing.product_id → 调 `BomService::suggest_materials` 得物料行 → 用 WMS 库存服务按 `product_id`+`warehouse_id` 叠加 `stock_qty` → 返回带库存的物料行。

### 6.4 新增端点（abt-web）
- `GET /admin/om/outsourcing/wo-summary`（B1）
- `GET /admin/om/outsourcing/suggest-materials`（B3，handler 内做 BomService + WMS 编排）
- `POST /admin/mes/orders/{order_id}/routings/{routing_id}/product`（A3 实例 product_id 编辑，与改价同守卫）

## 7. 数据流

```
routing_steps.product_id  --release 快照-->  work_order_routings.product_id
   (模板维护)                                       |
                                                   | 实例可编辑(首次报工前)
                                                   v
委外单选工序(routing_id) -----------------------> work_order_routings.product_id (半成品)
                                                   v
                          BomService.suggest_materials(product_id, planned_qty)
                                                   | BOM leaf × (1+loss) × qty + min_pack_qty
                                                   v
                          OM handler 叠加 WMS 库存(source_warehouse_id) --> 物料行
                                                   v
                          JS: min_pack_qty 整除校验 --> 允许/阻止提交
```

## 8. 错误处理

| 场景 | 处理 |
|---|---|
| 工序 product_id 为空 | suggest 返回空 + 提示「该工序未关联产出品，请先在工单工序维护」 |
| 半成品无已发布 BOM | suggest 返回空 + 提示「产出品无已发布 BOM 快照」 |
| `min_pack_qty` 为 0/NULL | 不校验（视为无包装约束） |
| 实例 product_id 编辑：已报工 | 守卫拒绝 `business_rule("该工序已报工，产出品不可修改")` |
| 跨工单 routing_id | `not_found` |
| min_pack_qty 非整除 | 前端 halt + 红字，后端提交时二次校验（防绕过） |

后端二次校验：`create_outsourcing_order` 提交时，对 `materials_json` 每行再查 `products.min_pack_qty` 校验整除，不满足 `validation` 拒绝（前端 halt 之外的安全网）。

## 9. 测试（DB 集成，`abt-web/tests/`，串行 `--test-threads=1`）

### Phase A
- 工序 product_id：模板存取（routing_create）、下达快照（release 后 work_order_routings.product_id == 模板值）、实例编辑守卫（未报工可改/报工后拒）
- products.min_pack_qty：存取

### Phase B
- `outsourcing_summary`：正确回填 product/qty/scheduled_end/customer
- `BomService::suggest_materials`：含 loss_rate 的需求量正确、空 BOM/空 product_id 分支、min_pack_qty 取值
- suggest-materials 端点：库存按 source_warehouse 叠加、未选仓库取合计
- min_pack_qty 校验：非整除被后端 `create_outsourcing_order` 拒绝

## 10. 设计文档同步

实现时同步 `docs/uml-design/04-mes.html`（`WorkOrderRouting` 加 product_id）、相关 OM/BOM uml 文档（`RoutingStep`/`Product`/`OutsourcingOrder` 加字段、`BomService.suggest_materials`）。

## 11. 不做（YAGNI）

- 不做「产品→销售单→客户」反查（取单规则不明）
- 不新建「工序-物料关联表」（用户明确要即时查询）
- 不做委外单详情页重设计（仅存 process_name 供展示）
- 不动 cost_entries labor 分录（已剥离到独立 cost-accounting spec）
- 不做产品维度的 min_pack_qty 批量导入（仅维护页输入框）

## 12. 实施顺序（计划阶段据此分阶段）

1. Phase A：migration 063 → 模型/repo → 下达传播 → 维护 UI（模板页 + 工单实例列 + product 编辑端点）
2. Phase B：outsourcing_summary 端点 → 工序下拉 → BomService.suggest_materials → suggest-materials 端点 → 物料行 + min_pack_qty 校验 → 后端二次校验
