# 生产计划 → 工单 下达流程评审报告

> **评审日期**：2026-06-14
> **评审角色**：产品经理 + 最终使用者
> **评审范围**：生产计划(ProductionPlan)下达为工单(WorkOrder)的完整业务流程
> **核心结论**：当前"一键下达"跳过了工单审核环节，主管失去对工单的控制力，流程不合理。

---

## 一、现状分析（Ground Truth）

### 1.1 当前流程

```
生产计划(Confirmed)
  │
  │  用户点击"确认并下达"
  │  release_to_work_orders() 自动执行：
  │    ① 对每个明细项 create() → 工单(Draft)
  │    ② 立即 release() → 工单(Released)
  │    ③ BOM快照冻结 + 工序创建 + 批次创建
  │
  ▼
工单(Released) × N  ← 直接进入可生产状态，主管无干预窗口
```

**代码位置**：`abt-core/src/mes/production_plan/implt.rs:231-322 release_to_work_orders()`
- 第 254 行：`work_order_svc.create()` — 生成 Draft 工单
- 第 287 行：`work_order_svc.release()` — **立即** Draft → Released
- 两步之间**没有任何人工审核环节**

### 1.2 对比：手动创建工单的流程

```
/admin/mes/orders/create → 工单(Draft) → 人工到详情页点"下达" → Released
```

**代码位置**：`abt-web/src/pages/mes_order_create.rs:55 create_order()` — 只 create，不 release
**代码位置**：`abt-web/src/pages/mes_order_detail.rs:343-350` — Draft/Planned 状态显示"下达工单"按钮

> **矛盾点**：手动创建的工单有 Draft 审核阶段；但计划下达的工单跳过了这个阶段。

### 1.3 工单当前能力盘点

| 操作 | Service 方法 | Web 路由 | 状态约束 |
|------|-------------|---------|---------|
| 创建 | `create()` | `POST /orders/create` | → Draft |
| 下达 | `release()` | `POST /orders/{id}/release` | Draft/Planned → Released |
| 反下达 | `unrelease()` | `POST /orders/{id}/unrelease` | Released → Draft |
| 关闭 | `close()` | `POST /orders/{id}/close` | → Closed |
| 取消 | `cancel()` | `POST /orders/{id}/cancel` | → Cancelled |
| 分割批次 | `split()` | `POST /orders/{id}/split` | — |
| **编辑** | **❌ 无** | **❌ 无** | — |
| **删除** | **❌ 无** | **❌ 无** | — |

---

## 二、产品经理视角（业务闭环与边界）

### 问题 P0-1：计划下达缺少"工单审核"环节，主管失去控制力

**现象**：点击"确认并下达"后，所有计划明细项 1:1 自动生成工单并立即进入 Released 状态。主管没有机会：
- 审核将要生成的工单
- 调整工单参数（排程日期、数量、工作中心、工艺路线）
- 选择性下达（只下部分明细项）
- 合并/拆分工单

**影响**：生产计划一旦确认，工单就不可逆地进入了生产流程。如果计划有误（排程冲突、产能不足），只能事后"反下达"或"取消"——代价大且留审计痕迹。

**行业标准做法**：
```
计划确认 → 生成工单建议(Draft) → 主管逐条审核/调整 → 选择性下达 → Released
```

**建议方案**：`release_to_work_orders()` 应该只执行 `create()`（生成 Draft 工单），不自动 `release()`。主管在工单列表/详情页逐个审核后手动下达。

**涉及文件**：
- `abt-core/src/mes/production_plan/implt.rs:287` — 删除自动 release 调用
- `abt-web/src/pages/mes_plan_detail.rs` — 下达后 modal 改为"已生成 N 个草稿工单，请到工单管理审核下达"

---

### 问题 P0-2：不支持"部分下达"

**现象**：`release_to_work_orders()` 对所有明细项遍历执行（`implt.rs:249 for item in &items`），无法选择只下达部分明细。

**场景**：计划有 5 个产品明细，但其中 2 个物料短缺，主管想先下达物料齐套的 3 个。

**当前行为**：预校验(modal)会显示物料短缺警告，但点击"确认下达"仍然尝试全部下达（失败的项进入 failed_items，成功的项正常生成）。

**建议方案**：
- **方案 A（推荐）**：在确认下达弹窗中，每个明细项前加 checkbox，默认勾选，允许取消勾选不下的项
- **方案 B**：保持全部下达逻辑，但只生成 Draft 工单（配合 P0-1），主管在工单列表选择哪些要 release

**涉及文件**：
- 方案 A：`abt-web/src/pages/mes_plan_detail.rs:348-405` modal 区域 + `abt-core/src/mes/production_plan/service.rs` 接口需增加 `item_ids: Option<Vec<i64>>` 参数

---

### 问题 P1-1：工单不支持编辑（Draft 状态下）

**现象**：`WorkOrderService` trait（`service.rs:10-64`）没有 `update()` 方法。Draft 状态的工单无法修改任何字段。

**影响**：
- 主管审核 Draft 工单时发现排程日期需要调整——改不了
- 工作中心分配错误——改不了
- 工艺路线需要替换——改不了
- 唯一选择：取消重建

**建议方案**：增加 `update()` 方法，仅允许 Draft/Planned 状态下修改。

**涉及文件**：
- `abt-core/src/mes/work_order/service.rs` — 新增 `update()` trait 方法
- `abt-core/src/mes/work_order/implt.rs` — 实现 update
- `abt-core/src/mes/work_order/repo.rs` — 新增 update SQL
- `abt-web/src/pages/mes_order_detail.rs` — Draft 状态显示编辑入口
- `abt-web/src/routes/mes_order.rs` — 新增编辑路由

---

### 问题 P1-2：手动创建工单表单字段缺失

**现象**：工单创建页（`mes_order_create.rs:43-54`）提交的 `CreateWorkOrderReq` 中：
- `routing_id: None` — 不能选工艺路线
- `sales_order_id: None` — 不能关联销售订单
- `bom_snapshot_id: None` — 无 BOM 选择
- 工作中心是裸数字输入框（`<input type="number">`），没有下拉选择

**影响**：手动创建的工单在 release 时会动态查找 Routing/BOM，但创建时用户无法指定，缺乏控制。

**建议方案**：创建表单增加工艺路线下拉（按产品过滤）、销售订单关联（可选）。工作中心改为下拉选择。

---

### 问题 P2-1：计划状态与明细状态不同步

**现象**：`release_to_work_orders()` 在 `implt.rs:308-311` 只要有一个成功就将计划标记为 `InProgress`。但部分失败时，计划变成了 InProgress，失败的明细项状态没有回退（仍为 Planned），用户难以追踪哪些下了哪些没下。

**建议方案**：计划详情页的"计划明细" tab 需要更清晰地标记每行的下达状态（已下达/未下达/失败），增加筛选维度。

---

## 三、最终使用者视角（操作效率与体验）

### 问题 U0-1："确认并下达"按钮名称误导

**现象**：按钮文字是"确认并下达"，用户以为只是确认操作。实际点击后所有明细立即自动生成工单并 release，进入不可逆的生产流程。

**吐槽**：我点了个"确认"，结果 5 个工单直接冲到生产线了？我连看都没看一眼工单长什么样！

**建议**：
- 如果保持自动 release：按钮改为"一键下达全部工单"，并加红色警告文字"将立即生成并下达 N 个工单"
- 如果改为生成 Draft（推荐）：按钮改为"生成工单"，modal 提示"已生成 N 个草稿工单，请到工单管理审核下达"

---

### 问题 U0-2：下达弹窗看不到将要生成什么

**现象**：确认下达的 modal（`mes_plan_detail.rs:348-405`）显示了预校验结果（BOM/工艺/物料），但没有展示将要生成的工单的关键参数：
- 工单编号（还没生成）
- 排程日期
- 工作中心
- 计划数量

**吐槽**：我看到"未配置工艺路线"的警告，但我看不到这个工单到底排到哪天、分到哪个工作中心。我是该点确认还是不该点？

**建议**：modal 中每个明细项展示：产品 · 数量 · 排程日期 · 工作中心，让用户在确认前看到全貌。

---

### 问题 U1-1：工单创建页"工作中心ID"是裸数字输入

**现象**：`mes_order_create.rs:82` — `<input type="number" name="work_center_id">`，用户需要手输工作中心 ID 数字。

**吐槽**：我怎么知道工作中心 ID 是几？去数据库查吗？

**建议**：改为下拉选择，列出所有工作中心名称。

---

### 问题 U1-2：工单列表缺少"来源计划"筛选

**现象**：工单有 `source_plan_id` / `source_plan_doc` 字段（`model.rs:31-32`），但工单列表页没有按来源计划筛选的功能。

**吐槽**：我从计划 PP-001 下达了 5 个工单，然后到工单列表想看这 5 个工单，但我没法按计划号筛选。

**建议**：工单列表筛选条件增加"来源计划编号"搜索。

---

### 问题 U2-1：反下达操作风险高但提示不够

**现象**：反下达（`unrelease()`）会删除生产批次和工序记录、取消领料单、释放库存。但反下达弹窗（`mes_order_detail.rs:412-437`）只有一段文字描述。

**吐槽**：我知道反下达有风险，但你至少告诉我具体会删掉什么？有几个批次？有多少报工记录？

**建议**：反下达弹窗中列出受影响的具体数据（N 个批次、M 条报工记录、K 张领料单），让主管明确知道代价。

---

## 四、整合修改方案

### 4.1 推荐目标流程（修改后）

```
生产计划(Confirmed)
  │
  │  用户点击"生成工单"（原"确认并下达"）
  │  release_to_work_orders() 只执行 create()：
  │    ① 逐个明细项 create() → 工单(Draft)
  │    ② 计划状态 → InProgress
  │
  ▼
工单(Draft) × N
  │
  │  主管到工单管理列表，看到 Draft 状态的工单
  │  逐个审核：
  │    - 可编辑参数（数量/日期/工作中心/工艺路线）
  │    - 确认无误 → 点"下达" → Released
  │    - 不需要的 → 点"取消" → Cancelled
  │
  ▼
工单(Released) × 已审核的子集
```

### 4.2 修改清单

| 序号 | 修改项 | 涉及文件/符号 | 修改类型 | 优先级 |
|------|--------|-------------|---------|--------|
| 1 | `release_to_work_orders()` 只 create 不 release | `abt-core/src/mes/production_plan/implt.rs:287` 删除 release 调用 | 修改 | **P0** |
| 2 | 计划下达 modal 文案改为"生成草稿工单" | `abt-web/src/pages/mes_plan_detail.rs:285,351,399-400` | 修改 | **P0** |
| 3 | 计划下达 modal 展示工单关键参数（排程/数量/工作中心） | `abt-web/src/pages/mes_plan_detail.rs:359-390` modal-body | 修改 | **P1** |
| 4 | 工单 Service 增加 `update()` 方法 | `abt-core/src/mes/work_order/service.rs` trait + `implt.rs` + `repo.rs` | 新增 | **P1** |
| 5 | 工单详情页 Draft 状态增加编辑入口 | `abt-web/src/pages/mes_order_detail.rs:330` page-actions + 新增编辑页/路由 | 新增 | **P1** |
| 6 | 工单创建表单增加工艺路线下拉 + 工作中心下拉 | `abt-web/src/pages/mes_order_create.rs:59-95` | 修改 | **P1** |
| 7 | 确认下达支持部分下达（checkbox 选择明细项） | `abt-web/src/pages/mes_plan_detail.rs` + `service.rs` 接口 | 新增 | **P2** |
| 8 | 工单列表增加"来源计划"筛选 | `abt-web/src/pages/mes_order_list.rs` | 修改 | **P2** |
| 9 | 反下达弹窗展示受影响数据明细（批次数/报工数） | `abt-web/src/pages/mes_order_detail.rs:412-437` | 修改 | **P2** |
| 10 | 计划明细 tab 每行显示下达状态（已下达/待下达/失败） | `abt-web/src/pages/mes_plan_detail.rs:412-478 tab_detail()` | 修改 | **P2** |

### 4.3 P0 项的最小改动方案（立即可做）

**只改 2 个文件，解决核心问题**：

**文件 1**：`abt-core/src/mes/production_plan/implt.rs`
```rust
// release_to_work_orders() 第 278-304 行
// 删除自动 release，只保留 create

// 改前：create → release 两步
// 改后：只 create，不 release
```

具体：将第 278-304 行的 release 逻辑删除，create 成功后直接更新 PlanItem 状态为 Released（或新增一个中间状态 Generated），不调用 `work_order_svc.release()`。

**文件 2**：`abt-web/src/pages/mes_plan_detail.rs`
- 按钮"确认并下达" → "生成工单"
- Modal 标题"确认下达生产计划？" → "生成生产工单"
- Modal 确认按钮"确认下达" → "生成草稿工单"
- Modal 描述改为"将根据计划明细生成草稿工单，请到工单管理审核并下达"

这样改完后：
- 主管点击"生成工单" → 生成 Draft 工单
- 主管去工单管理列表 → 看到 Draft 工单
- 主管逐个审核 → 点"下达" → Released
- 不需要的 → 点"取消"

**无需新增 Service 方法或路由**——现有的 `create()` + 工单详情页的 `release` 按钮已经能支撑这个流程。

---

## 五、设计文档同步要求

以下修改如被采纳，需同步更新设计文档：

| 设计文档 | 需更新内容 |
|---------|-----------|
| `docs/uml-design/04-mes.html` | `ProductionPlanService.release_to_work_orders()` 的语义说明：从"创建并下达"改为"仅创建草稿工单" |
| `docs/uml-design/04-mes.html` | `WorkOrderService` 增加 `update()` 方法定义（若采纳 P1-1） |

---

## 六、风险与注意事项

1. **已有数据兼容**：已有的 Released/InProgress 状态的工单不受影响，改动只影响新下达的计划。
2. **权限**：工单审核下达需要 `WORK_ORDER.update` 权限（已有），无需新增权限。
3. **若选择部分下达（P2-7）**：需修改 `release_to_work_orders()` 接口签名，增加 `item_ids` 参数，属于 breaking change。
4. **计划状态语义变化**：改为只生成 Draft 后，计划状态 `InProgress` 的含义从"工单已下达生产"变为"工单已生成待审核"。需确认是否引入新的计划状态（如 `Generated`）或在现有状态下通过明细状态区分。
