# 委外单创建页优化 — 设计方案（Issue #67）

> **状态**：设计草案 / 待主分支实施
> **来源**：GitHub Issue #67 — `[OM委外] 委外单创建页功能优化`
> **页面**：`/admin/om/outsourcing/create`（`abt-web/src/pages/om_outsourcing_create.rs`）
> **讨论分支**：`feat/voucher-currency`（本分支不实施，结论移交主分支）
> **整理日期**：2026-06-21

---

## 1. Issue 三个核心需求回顾

| # | 需求 | 一句话目标 |
|---|------|-----------|
| 1️⃣ | 基本信息多源联动 | 选「关联工单」时自动带出 产品 / 计划数量 / 交期 / 客户 等 |
| 2️⃣ | 关联工序显示工序名 | 把「关联工序」从数字输入框换成显示工序名（贴片/插件/组装…）的下拉 |
| 3️⃣ | 发料明细跨条件联动 + 最小包装数校验 | 选定「产品+工序+订单」后即时带出 物料/名称/需求量/库存；需求量须能被 `min_pack_qty` 整除，否则阻止提交 |

---

## 2. 现状代码分析（基于 feat/voucher-currency 分支）

### 2.1 前端 `om_outsourcing_create.rs`
- 表单字段（`CreateForm`）：`supplier_id`、`product_id`、`outsourcing_type`、`work_order_id`、`routing_id`、`planned_qty`、`unit_price`、`scheduled_date`、`virtual_warehouse_id`、`source_warehouse_id`、`remark`、`materials_json`。
- **关联工序**当前是一个裸数字输入框 `<input name="routing_id" placeholder="请输入工序ID">` —— 这是需求2要修的点。
- **发料明细**通过 Modal 选物料 + 行内 JS 拼装 `materials_json`，**无任何跨条件联动 / 包装校验**。
- 页面加载时一次性把 suppliers / products / warehouses / work_orders 全 list（page 200）。

### 2.2 后端 OM 模型 `abt-core/src/om/outsourcing_order/model.rs`
- `OutsourcingOrder` 含 `work_order_id: Option<i64>`、`routing_id: Option<i64>`、`product_id: i64`、`planned_qty`、`unit_price`、`scheduled_date`。
- `OutsourcingMaterialItem { product_id, planned_qty, unit_cost }` —— 发料明细行。
- `OutsourcingOrderService` 仅有 CRUD + send/receive/convert/cancel + `list_materials` + `list_inventory_records`，**缺**：按"工单+工序"查询物料需求、查询工单工序列表等联动接口。

### 2.3 关键数据模型（已存在，可直接复用）
- **`work_orders`（工单）**：`product_id`、`planned_qty`、`scheduled_start`、`scheduled_end`、`routing_id`、`sales_order_id`，以及列表聚合字段 `source_customer`、`source_so_doc`、`completed_steps`/`total_steps`。→ 需求1联动数据的**主要来源**。
- **`work_order_routings`（工单级工序实例）**：`id`、`work_order_id`、`step_no`、`process_name`、`work_center_id`、`unit_price`、`is_outsourced`、`is_inspection_point`、`planned_qty`… → 需求2的**直接数据源**（已有 process_name、已有 unit_price）。
- **BOM**：`boms` + `bom_nodes`（树：`product_id`、`parent_id`、`quantity`、`loss_rate`、`unit`…）+ `bom_snapshots`（已发布快照）。→ 需求3物料来源。
- **`routing_steps`（主数据工序模板）** 与 **`work_order_routings`（工单工序实例）** 是两层：工单下达时由 routing 模板快照生成 work_order_routings。

### 2.4 ⚠️ 数据缺口（影响需求3）
- **`bom_nodes` 没有任何工序字段**（仅 `work_center` 字符串）。无法直接从 BOM 查"某道工序需要哪些料"。
- **`work_order_routings` 没有 `product_id`** —— 工序实例不记录它加工/产出的半成品。
- **`min_pack_qty`（最小包装数量）字段在全系统不存在**。

---

## 3. 已达成的设计决策（本次讨论确认）

### 需求1 — 基本信息联动：**方案 A（以工单为主）**
- 触发：选择「关联工单」时。
- 自动回填：`product_id`（工单产品）、`planned_qty`（工单计划数量）、`scheduled_date`（=工单 `scheduled_end`）、客户名（只读展示 `source_customer`）。
- 轻联动：选「产品」时按 `product_id` 过滤「关联工单」下拉，并带出该产品 BOM 关联的工艺路线。
- **不**做"产品→销售单→客户"的反查（取单规则不明确，YAGNI）。

### 需求2 — 关联工序显示工序名：**方案 A**
- 「关联工序」由数字输入框 → **下拉**，选项 = 所选关联工单下的 `work_order_routings`（`process_name`），且只展示 `is_outsourced = true` 或全部工序（待定，见 §5）。
- 提交值 = `work_order_routings.id`（即现有 `routing_id` 字段的语义，**无需改表结构**）。
- 同时把所选工序的 `process_name`、`unit_price` 一并存到 OM 单上便于详情页展示（**需给 `outsourcing_orders` 加 `process_name` 冗余列**，见 §6）。

### 需求3 — 发料明细跨条件联动 + min_pack_qty 校验：**方案 2（工序→半成品→BOM 即时查询）**
- 核心思路（用户确认）：**选定工序后，应知道它在加工哪个半成品**，进而即时查询该半成品的 BOM 得到所需物料。
- 即时查询链路：`工序(work_order_routings.id) → 该工序关联的半成品 product_id → 该半成品的已发布 BOM 快照叶子节点 → 物料 + 需求数量`。
- 需求数量 = `bom_nodes.quantity × (1 + loss_rate) × 订单计划数量(planned_qty)`。
- 库存数量 = 按物料 `product_id` 查库存（仓库取 `source_warehouse_id`，接口待定）。
- **不**新建"工序-物料关联表"；通过给工序加 `product_id` 外键 + 运行时 JOIN 实现（用户明确要求"即时查询，不要关联表"）。
- min_pack_qty 校验：物料基础数据有 `min_pack_qty` 时，用户输入的「需求物料数量」须 `qty % min_pack_qty == 0`；失焦/提交时红字提示 *"需求数量必须是最小包装数量 [X] 的整数倍"* 并阻止提交。

---

## 4. 待主分支确认的开放问题（实施前必须拍板）

1. **需求3 的 `product_id` 加在哪张表？**（二选一）
   - (a) 加到 `work_order_routings` —— 工单级工序实例直接记录半成品。**推荐**：与"创建工单时设计工序和价格"的维护流程同处，OM 选的是工单工序实例，链路最短。
   - (b) 加到 `routing_steps`（主数据工序模板）—— 模板级，所有引用该模板的工单共享。
   - **倾向 (a)**，但需主分支结合「工单工序计件单价维护」功能（近期提交 `87745b71`/`cb9efdc1`）一并决策。

2. **`min_pack_qty` 存储位置？**（三选一）
   - (a) `products.min_pack_qty` 独立列（migration ALTER TABLE）—— **推荐**，与 unit/acquire_channel 等物料属性并列，便于查询/校验。
   - (b) 塞进 `Product.meta` JSONB —— 改动小但语义弱。
   - (c) 独立物料包装主数据表 —— 过度设计。
   - 采用 (a) 时需同步：Product 实体、Product 维护页输入框、`docs/uml-design/`。

3. **需求2 下拉是否只列 `is_outsourced = true` 的工序？** 业务上"工序委外"通常只外包标记为可委外的工序，建议默认只列可委外工序，并提供"全部"切换。

4. **需求3 的库存数量取哪个仓库？** 建议取表单已选的「发料源仓库 `source_warehouse_id`」；若未选则取该物料全部在库库存合计。

5. **需求3 触发时机：** 选定「工序」+「计划数量」后自动查询填充物料行；用户可在此基础上手动增删/改量（最终仍受 min_pack_qty 校验）。

6. **联动查询是否需要新接口？** 倾向新增 2 个端点（见 §7），而非在前端硬拼。

---

## 5. 建议的数据模型变更（migration）

> 待主分支按 §4 拍板后落地。编号占位。

```sql
-- m1: 委外单冗余工序名（需求2 详情页展示）
ALTER TABLE outsourcing_orders ADD COLUMN IF NOT EXISTS process_name VARCHAR(200);

-- m2: 工序实例关联半成品（需求3 链路核心，方案4-a）
ALTER TABLE work_order_routings ADD COLUMN IF NOT EXISTS product_id BIGINT REFERENCES products(product_id);

-- m3: 物料最小包装数量（需求3 校验核心，方案4-a）
ALTER TABLE products ADD COLUMN IF NOT EXISTS min_pack_qty DECIMAL(18,6);
```

同步：对应 Rust 实体（`OutsourcingOrder`、`WorkOrderRouting`、`Product`）+ `docs/uml-design/`。

---

## 6. 建议的接口设计（abt-core Service trait）

> 遵循项目「接口与模型先行」，先定 trait 再实现。命名草案。

### 6.1 工单工序列表（需求1/2 联动用）
挂在 `WorkOrderService` 或 `ProductionBatchService`（已持有 `WorkOrderRoutingRepo`）：
```rust
async fn list_work_order_routings(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64
) -> Result<Vec<WorkOrderRouting>>;   // 含 process_name / unit_price / is_outsourced / product_id
```
前端据此渲染「关联工序」下拉。

### 6.2 委外发料物料即时查询（需求3 核心）
挂在 `OutsourcingOrderService`（或 BOM service）：
```rust
pub struct OutsourcingMaterialSuggestionItem {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub required_qty: Decimal,     // = bom用量×(1+损耗)×计划数量
    pub stock_qty: Decimal,        // 指定仓库库存
    pub min_pack_qty: Option<Decimal>,
}

pub struct OutsourcingMaterialQuery {
    pub work_order_routing_id: i64,  // 工序实例 → 取其 product_id(半成品)
    pub planned_qty: Decimal,        // 订单计划数量
    pub warehouse_id: Option<i64>,   // 发料源仓库，None=全仓库合计
}

async fn suggest_outsourcing_materials(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>, q: OutsourcingMaterialQuery
) -> Result<Vec<OutsourcingMaterialSuggestionItem>>;
```
查询逻辑：`routing → product_id(半成品) → 该半成品已发布BOM快照 leaf nodes → 各物料 × 计划数量 → JOIN 库存 + min_pack_qty`。

### 6.3 工单联动摘要（需求1，可选独立端点）
若不想前端逐字段拼，可加：
```rust
pub struct WorkOrderOutsourcingSummary {
    pub product_id: i64, pub planned_qty: Decimal,
    pub scheduled_end: Option<NaiveDate>, pub customer_name: Option<String>,
    pub routings: Vec<WorkOrderRouting>,
}
async fn outsourcing_summary(&self, ..., work_order_id: i64) -> Result<WorkOrderOutsourcingSummary>;
```

---

## 7. 前端交互落地（abt-web，待接口就绪后实施）

- **关联工单 `change`** → hx-get 调 §6.3（或分两次调 §6.1）→ setFieldsValue 回填 产品/数量/交期/客户 + 重渲染「关联工序」下拉。
- **关联工序 `change`** + **计划数量 `change`** → hx-get 调 §6.2 → 渲染物料行表格（物料编码/名称/需求量/库存/min_pack_qty）。
- **min_pack_qty 校验**：物料行 `<input>` 失焦 + 表单 submit 时，JS 校验 `qty % min_pack_qty === 0`，不满足则红字提示并 `halt` 提交（符合 abt-web「纯前端 UI 用 Hyperscript / 复杂逻辑用 script」约定）。
- 遵循 `abt-web/CLAUDE.md`：TypedPath、`hx-target`、单端点、禁止 fetch 提交、UnoCSS 原子类。

---

## 8. 与近期在途工作的衔接

- 提交 `87745b71 docs(mes): 工单工序计件单价维护 + 工序删除设计`、`cb9efdc1 docs(mes): 设计补充 wage_amount 冻结 + 成本核算范围剥离` 正在重构工单工序（`work_order_routings`）的维护流程。
- **建议**：§4-问题1（product_id 加表位置）与该在途工作合并设计，避免对 `work_order_routings` 两次改表、两次返工。

---

## 9. 实施顺序建议（主分支）

1. 拍板 §4 全部开放问题（尤其问题1、2）。
2. migration（§5）+ 实体 + `docs/uml-design/` 同步。
3. abt-core 接口 §6（先 trait 评审）。
4. abt-web 前端 §7。
5. 验证：`cargo clippy` + 页面 `snapshot -i` 走查（禁止截图）。
6. 回 Issue #67 评论修复内容 + 关联提交，等用户确认后关闭。
