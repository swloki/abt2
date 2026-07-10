# 工艺路线解耦设计（Routing Decouple）

> **状态**：设计已确认（2026-07-08 用户拍板 4 决策点）· **单 PR · clean break**，进入实现
> **目标**：把 migration 045/063 焊进 `routing_steps` 的「产出品 `product_id`」与「计件单价 `unit_price`」下沉回 per-BOM 覆盖层，恢复 `routing` 为**纯工艺模板**，回归 `09-master-data` 既定的「三层工艺解耦」原设计。
> **来源**：ERPNext / Odoo / OFBiz 三家 ERP 横向对比（ultracode workflow w12ovzc6r，7-agent）+ abt_v2 现网 DATA AUDIT（2026-07-08）。

---

## 1. 问题

### 1.1 三正交维度被焊在一行

`routing_steps`（工艺模板行）同时承载了三个**本应分离**的维度，任一差异即整条 routing 分叉：

| 维度 | 字段 | 本质 | 跨产品可共享 |
|---|---|---|---|
| 工艺结构 | `process_code` / `step_order` / `work_center_id` / `standard_time` / `is_outsourced` / `is_inspection_point` | "焊接 5 分钟、需 IPQC" | ✅ |
| **产出语义** | `product_id`（迁移 063 引入） | "该工序产出哪个中间品" | ❌ 属于具体 BOM |
| **计件价格** | `unit_price`（迁移 045 引入） | "该工序计件工资" | ❌ 商务/产品特定 |

### 1.2 历史溯源

`bom_labor_processes`（迁移 013）**本就是 per-BOM 计件价表**（`product_code + labor_process_dict_id + unit_price + quantity`，迁移 045 注释明写"中国制造业计件工资特色"）。045 把 `unit_price` 复制到 `routing_steps`、063 又把 `product_id` 复制过去——**是这两次复制制造了耦合，而非 ABT 缺覆盖层**。

### 1.3 现网数据实证（abt_v2，2026-07-08）

| 指标 | 实测 | 说明 |
|---|---|---|
| bom_routings 分布 | RT000001=388、RT000002=130、RT000007=3、其余 13 条各绑 1 | 80% 绑定 routing 为 1:1 per-BOM |
| RT000001 同族性 | 388 BOM 全是"注塑模组/滴胶模组"不同型号 | ✅ 真·同构家族，模板复用合理 |
| **RT000002 模板** | **19 step 全 NULL product_id、全 0 unit_price、全 NULL work_center** | 🔴 空壳 routing，无工艺数据 |
| **work_order_routings 快照** | **3820 行仅 73 行有 product_id（98.1% 断裂）** | 🔴 现网 bug，OM 委外/工序领料读不到产出品 |
| RT000001 工单快照 | 770 行，**0 行有 product_id** | 🔴 migration 063 前生成的快照永久 NULL，无回填机制 |
| bom_labor_processes | 6994 行 / 534 产品 | 真实人工成本数据源，不可无视 |

---

## 2. 三家 ERP 横向对比与四条铁律

| 维度 | ERPNext | Odoo (mrp) | OFBiz |
|---|---|---|---|
| 产出品归属 | `BOM.item`，绝不在 operation | `mrp.bom.product_id` + `bom.byproduct`，绝不在 operation | 带类型边表 `WorkEffortGoodStandard` |
| 价格/工时 | 价 `workstation.hour_rate`，时在 BOM operation（**彻底分离**） | 价 `workcenter.costs_hour`，时 `time_cycle_manual`（**彻底分离**） | 时挂 WorkEffort，费率 `CostComponentCalc`（**彻底分离**） |
| 工序复用 | **全局可复用** Operation 主数据 | 拷贝语义（从属 BOM） | **全局可复用** ROU_TASK（边表多对多） |
| BOM-routing 关联 | 松耦合可选 Link + 快照拷贝 | operation 完全内联 BOM | 解耦多对多边表 |

**四条铁律**（三家无一例外）：
1. **产出品绝不在 routing/operation 主数据上**
2. **价格（费率）归工作中心，工时归工序**，运行时相乘；调价只改工作中心，不动工艺主数据
3. 复用靠**引用/快照**，不靠字段堆叠
4. **copy-on-write 是可接受取舍**（模板变更不自动回流到已建工单）

ABT 已具备两个被低估的解耦基础设施：
- `labor_process_dicts` ≡ ERPNext `Operation` / OFBiz `ROU_TASK`（**全局工序库已存在**）
- `work_center.costs_hour` 已是 Odoo 式制费（`production_batch/implt.rs:294-299` 已正确计算 `overhead = work_hours × costs_hour`）

> **China 特色**：三家 ERP 都是工时制，ABT 的 `unit_price` 是计件工资——既不能纯归工作中心（同机器不同产品计件不同），也不该留模板（产品特定），唯一正确归属是 **per-BOM-per-step**。

---

## 3. 目标模型设计（Interface & Model First）

### 3.1 实体变更总览

| 实体 | 变更 | 角色 |
|---|---|---|
| `routing_steps` | **瘦身**：`product_id`/`unit_price` 降级为可空模板默认 | 纯工艺结构模板（工序序列 + 工作中心 + 工时 + 工艺标记） |
| `bom_routing_outputs`（**新增**） | per-BOM-per-step 产出 + 计件覆盖 | "这个 BOM 在这道工序产出哪个中间品、计件多少"（≡ Odoo `bom.byproduct.operation_id` / OFBiz `WorkEffortGoodStandard`） |
| `bom_labor_processes` | **保持 legacy**，角色收敛为"无 routing 的独立人工工序" | 按 `quantity` 计费的 Excel 导入人工（无工作中心/工时/委外语义） |
| `work_order_routings` | **列不变**，只改 `load` 数据来源 | 工单工序快照（copy-on-write 断层） |
| `bom_routings` | 不变 | 1 BOM : 1 routing 绑定（保留，不引入多 routing 叠加） |

### 3.2 `bom_routing_outputs`（新增覆盖层）

```sql
CREATE TABLE bom_routing_outputs (
    id                  BIGSERIAL PRIMARY KEY,
    product_code        VARCHAR(100) NOT NULL,        -- 成品编码（与 bom_routings.product_code 对齐）
    routing_id          BIGINT NOT NULL REFERENCES routings(id),
    step_order          INT NOT NULL,                  -- 对齐 routing_steps.step_order（见 §6.1 关联键）
    output_product_id   BIGINT REFERENCES products(product_id),  -- 该工序产出的中间品；必须 ∈ 该 BOM 非叶子节点
    unit_price          NUMERIC(18,6),                 -- 该 BOM 该工序计件单价（空 → 回退模板/报"未定价"）
    work_center_id      BIGINT REFERENCES work_centers(id),      -- 可空覆盖；空 → 用模板 routing_steps.work_center_id
    operator_id         BIGINT,
    created_at          TIMESTAMPTZ DEFAULT now(),
    updated_at          TIMESTAMPTZ DEFAULT now(),
    UNIQUE (product_code, step_order)                  -- 一个 BOM 的一道工序最多一个产出映射
);
```

> **为何不演进 `bom_labor_processes` 而新建表**：审计 + 代码核查（`bom/implt.rs:819-840 build_labor_from_legacy`）证实 `bom_labor_processes` 带 `quantity`（按数量计费）、无 `work_center_id`/`standard_time`/`is_outsourced`（无工艺属性），与"routing 产出覆盖"**计费模型不同**（计时 vs 按数量）。强行合并会丢信息。双轨按"是否有 routing"切分（见 §3.4）。

### 3.3 `routing_steps` 字段语义变更

| 字段 | 旧语义 | 新语义 |
|---|---|---|
| `product_id` | 强制非空、必须 ∈ 绑定 BOM 非叶子节点 | **可空模板默认**（仅 RT000001 式同构家族沿用） |
| `unit_price` | 强制非空 | **可空模板默认**（同上） |
| 其余（work_center_id/standard_time/is_outsourced/is_inspection_point/allowed_loss_rate） | 不变 | 不变（真正可共享的工艺属性） |

> **clean break**：实现末尾 DROP `product_id`/`unit_price` 两列，全量回填到 `bom_routing_outputs`（§4.1 M2）。RT000001 同构家族的 388 BOM 各生成 8 行覆盖行（`output_product_id`/`unit_price` 取自原模板值）。

### 3.4 取数优先级矩阵（消灭多源歧义）

核心规则：**按 BOM 是否绑定 routing 切分，`unit_price`/产出品在两条路径下不重叠**。

| BOM 情形 | 产出品 / 计件价取数 | 说明 |
|---|---|---|
| **绑定 routing** | `bom_routing_outputs`（M2 回填保证每 BOM 每工序齐全） | 单一源，无回退（RT000002 空壳除外，保持现状空值） |
| **未绑定 routing** | `bom_labor_processes`（legacy 独立人工） | 无 routing 的产品，按 quantity 计费 |

→ **不存在三源并存**：有 routing 走覆盖链，无 routing 走 legacy，互斥切分。

### 3.5 Service trait（接口先行）

新增独立 trait，**不塞进已 11 方法的 `RoutingService`**（避免接口膨胀），放 `master_data/bom_routing_output/` 独立四文件模块：

```rust
#[async_trait]
pub trait BomRoutingOutputService: Send + Sync {
    /// 列出某 BOM 绑定 routing 的全部工序 + 覆盖状态（前端编辑分区用）
    async fn list_steps_with_output(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>, product_code: String
    ) -> Result<Vec<StepWithOutput>>;

    /// UPSERT 单道工序的产出覆盖（by product_code + step_order）
    async fn upsert_output(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>, req: UpsertBomOutputReq
    ) -> Result<()>;

    /// 删除单道工序的产出覆盖（回退模板默认）
    async fn delete_output(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>, product_code: String, step_order: i32
    ) -> Result<()>;
}
```

`RoutingService` trait **不变**；`load_routings_from_template` 改签名（见 §3.6）。

> **校验归属**：产出品"∈ 该 BOM 非叶子节点"校验放在 **abt-web handler 层**（持有 BomQueryService + 新 service 双句柄，天然解耦），或在 routing repo 直接读 `bom_nodes`（CLAUDE.md 允许 crate 内部跨模块 repo 直访）。**禁止**在 `BomRoutingOutputService` 上反向依赖 `BomQueryService`（避免 service 层循环依赖）。

### 3.6 `work_order_routings` load 路径切换

`load_routings_from_template`（`production_batch/implt.rs:579-638`）改造：

- **签名**：`(work_order_id, routing_id)` → `(work_order_id, routing_id, product_code)`
  - `product_code` 由调用方 `WorkOrderService::create/release` 从 `work_order.product_id` 解析传入（`work_orders.product_id` 已存在，零额外查询）
- **INSERT 列不变**（快照列零侵入，下游报工/领料/委外零改）
- **`product_id` / `unit_price` / `work_center_id` 数据来源**：读 `bom_routing_outputs`（按 `product_code + step_order`）。clean break 后模板列已 DROP，覆盖行由 M2 回填保证齐全（RT000002 空壳除外）

---

## 4. 实现路径（单 PR · clean break）

一次性完成，按 数据层 → 后端 → 前端 → 收尾 顺序。

### 4.1 数据层（migration）

**M1 建表**：`CREATE TABLE bom_routing_outputs`（§3.2）

**M2 数据回填**（产出品/计件价从模板搬到覆盖层 + 顺带修快照断裂）：

```sql
-- (a) 生成 per-BOM 覆盖行：每个绑定的 BOM × 该 routing 的 steps
INSERT INTO bom_routing_outputs (product_code, routing_id, step_order, output_product_id, unit_price, work_center_id, created_at)
SELECT br.product_code, br.routing_id, rs.step_order, rs.product_id, rs.unit_price, rs.work_center_id, now()
FROM bom_routings br
JOIN routing_steps rs ON rs.routing_id = br.routing_id
WHERE rs.product_id IS NOT NULL;   -- RT000002 空壳无 product_id → 不生成（保持现状）

-- (b) 回填历史快照断裂（并入原 PR-A）
UPDATE work_order_routings wor
SET product_id = rs.product_id
FROM work_orders wo
JOIN routing_steps rs ON rs.routing_id = wo.routing_id AND rs.step_order = wor.step_no
WHERE wor.work_order_id = wo.id AND wor.product_id IS NULL AND rs.product_id IS NOT NULL;
```

RT000001 同构家族：388 BOM × 8 step = 3040 行覆盖；RT000002 模板空 → 覆盖行为空（业务后续补，不阻塞）。

**M3 DROP 列**（grep 确认 `routing_steps.product_id`/`unit_price` 无下游读后）：

```sql
ALTER TABLE routing_steps DROP COLUMN product_id;
ALTER TABLE routing_steps DROP COLUMN unit_price;
```

### 4.2 后端

- 新增 `master_data/bom_routing_output/` 四文件模块 + `BomRoutingOutputService` trait（§3.5）
- `load_routings_from_template` 改签名 `+ product_code` + 读覆盖层（§3.6，clean break 无回退）
- `try_build_labor_from_routing`（`bom/implt.rs:795`）/ OM 委外（`om_outsourcing_create.rs:251/367/411`）：读覆盖层或 `work_order_routings` 快照
- routing 编辑约束：对"有覆盖"的 routing 禁止破坏性 step 重排（后端守卫，参照已报工锁定先例）
- `find_matching_routing` 复合判据（`process_code` + `work_center_id` + `standard_time` 容差）+ Web 复用入口

### 4.3 前端

- `routing_create.rs` 工序步骤表**移除"产出品/计件单价"两列** + 移除 `parse_steps` 对应校验（131-144）+ 移除 `compute_output_candidates`（195-214）/ `RoutingOutputSearchPath`（240-283）整条搜索链路 → **彻底消灭 Issue #212 候选裁剪**
- **routing 详情页**新增"按关联 BOM tab → per-step 产出/计件编辑"分区（产出品 picker 候选 = 该 BOM 自身非叶子节点）
- InProduction 工单 reload 守卫

### 4.4 收尾

- `cargo clippy` + `cargo test`
- 同步 `09-master-data.html`（标注 `bom_routing_outputs` 实体 + `routing_steps` 字段变更 + labor cost 取数链路）

---

## 5. 护栏与风险

### 5.1 step 关联键（高风险）

`routing` update 是 `delete_steps + insert_steps` 全重建（`repo.rs:52-58`），`step_order` 不稳定 → 覆盖行 `step_order` 会错位（把"焊接单价"挂到"测试"上）。

**推荐方案（编辑约束，参照 `implt.rs:600-602` 已报工 step 锁定先例）**：
- 对"已有 `bom_routing_outputs` 覆盖"的 routing，编辑时**禁止破坏性 step 重排/删除**（前端提示 + 后端守卫）
- 仅允许在末尾 append 新工序

备选：引入 `step_key`（UUID，insert 生成，全重建时按 `process_code` 旧→新映射保留）——彻底但改动大，本期不采用。

### 5.2 InProduction 工单保护

`load_routings_from_template` 现允许 `Draft | Released | InProduction` 重新加载（588）。PR-B 切换数据源期间，对 `InProduction` 工单禁止 reload（避免同一工单前后两次 load 得到不同 `product_id`，在制品成本快照不一致）。

### 5.3 多源一致性（已由 clean break 消除）

采纳 clean break：M3 DROP `routing_steps.product_id`/`unit_price` 后，产出品/计件价唯一源是 `bom_routing_outputs`（有 routing）或 `bom_labor_processes`（无 routing），按 §3.4 切分，**无多源歧义**。前提：M2 回填须保证"有 routing 的 BOM"覆盖行齐全，否则 load 取不到值——M2 后用 SQL 校验覆盖行数 ≈ 绑定 BOM × steps。

### 5.4 standard_cost 死字段

核实 `routing_steps.standard_cost` 无运行时消费方（仅 INSERT/SELECT 搬运）。建议 PR-B 一并 DROP（待 grep 最终确认无下游读）。

---

## 6. 决策记录（2026-07-08 用户确认）

1. **覆盖层落点** → ✅ 新建 `bom_routing_outputs`（双轨，`bom_labor_processes` 保持 legacy）。
2. **step 关联键** → ✅ 编辑约束（对"有覆盖"的 routing 禁止破坏性 step 重排，仅允许 append）。
3. **PR 节奏** → ✅ 单 PR 一次性完成（原 PR-A 快照回填并入 M2）。
4. **`routing_steps.product_id`/`unit_price` 终态** → ✅ clean break：M3 DROP 列 + M2 全量回填覆盖层。

---

## 7. 实现进度与下个会话接续

**分支**：`feat/routing-decouple` ｜ **状态**：clean break 编译闭环完成（`cargo clippy -p abt-web --all-targets` 通过），已 commit 未推远程。

### 7.1 已完成（本会话）
- **后端**：`bom_routing_output` 模块（`BomRoutingOutputService` trait + factory + `master_data/mod.rs` 注册）；migration 096（建表 + 回填覆盖层 + 修历史 `work_order_routings` 快照断裂）、097（DROP `routing_steps.product_id/unit_price`）；`load_routings_from_template` 改读覆盖层（签名 `+product_code`）；`try_build_labor_from_routing` 改读覆盖层；`RoutingStep`/`RoutingStepInput` 删 `product_id/unit_price/product_name`；`routing/repo` INSERT/SELECT 清理。
- **前端 lib**：`routing_create` 瘦身（删产出/价格 UI+JS+picker+校验，整体重写）；`routing_detail` 删产出/价格列；`routes` 删 `RoutingOutputSearchPath`。
- **测试编译**：`routing_labor_e2e.rs` / `mes_routing_price.rs` 适配 load 新签名。
- **下游零侵入**：om 委外 / 工序领料读 `work_order_routings` 快照，确认无需改。

### 7.2 剩余（下个会话）
1. **routing_detail 覆盖层编辑 UI**（核心剩余，新 CRUD）：
   - 关联 BOM 区加"维护产出/计件"展开交互（选 BOM → 拉 `list_steps_with_output` → per-step 编辑）
   - 产出品 picker 候选 = **该 BOM 自身非叶子节点**（按 `product_code` 后端算，复用旧 `get_routing_output_search` 逻辑但维度从 `routing_id` 改 `product_code`；候选集可能很大，后端过滤）
   - per-step：产出 picker + 计件单价 input + 工作中心覆盖 select（空=用模板）→ `upsert_output` / `delete_output`
   - 新 handler + TypedPath 路由
   - 后端 trait 已就绪（`abt_core::master_data::bom_routing_output`）：`list_steps_with_output(ctx,db,product_code)` / `upsert_output(ctx,db,UpsertBomOutputReq)` / `delete_output(ctx,db,product_code,step_order)`
2. **编辑约束**：routing update 对"有 `bom_routing_outputs` 覆盖"的 routing 禁止破坏性 step delete/reorder（参照 `production_batch/implt.rs:600-602` `has_report` 锁定先例）
3. **测试运行时逻辑**：`routing_unit_price_carries_to_work_order_on_load`（routing_labor_e2e.rs:145）断言需先 upsert `bom_routing_outputs` 再验证流入；`routing_create_rejects_missing_unit_price/product_id`（:73/:96）验证的是已删校验，应删
4. **文档同步**：`09-master-data.html` 标注 `bom_routing_outputs` 实体 + `routing_steps` 字段变更 + labor cost 取数链路
5. **migration 执行**：部署时手动 `psql -f 096_*.sql && psql -f 097_*.sql`（096 必须先于 097）

### 7.3 下个会话第一步
1. `git checkout feat/routing-decouple`，读本节 + §3-5（模型/接口/取数/落地）
2. `cargo clippy -p abt-web --all-targets` 确认编译基线绿
3. 先加覆盖层 UI 路由 + handler（`list_steps_with_output` 渲染 + upsert/delete），再加 `routing_detail` 页面交互
4. 测试运行时逻辑 + 编辑约束 + `09-master-data.html` 同步收尾

---

## 关联

- [`09-master-data.html`](09-master-data.html) — 三层工艺解耦原设计（本方案回归点）
- [`README.md`](README.md) — 共享基础设施接口规范（DomainEventBus/AuditLog/PaginatedResult）
- v5 事件 `BomRoutingChanged`（README:237）可用于覆盖层变更后的局部刷新广播
